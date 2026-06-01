use std::path::Path;
use image::{self, imageops::FilterType};
use image::ImageReader;
use crate::shared::error::AppResult;

/// 缩略图处理的源图内存上限（字节）: 30MB
/// 4000×3000 RGBA = 48MB → 超过上限，跳过
/// 2000×1500 RGBA = 12MB → 低于上限，可以处理
const MAX_SOURCE_IMAGE_MEMORY_BYTES_FOR_THUMBNAIL_GENERATION: u64 = 30_000_000;

/// 缩略图生成配置
pub struct ThumbnailGenerationConfig {
    /// 目标宽度列表（px），只能缩小不能放大
    pub widths: Vec<u32>,
    /// 是否保留原图（当前实现始终保留，此字段预留）
    pub keep_original: bool,
}

/// 单个缩略图信息
pub struct ThumbnailInfo {
    /// 尺寸标签，如 "400w"
    pub size_label: String,
    /// 实际宽度（px）
    pub width: u32,
    /// 实际高度（px，等比缩放）
    pub height: u32,
    /// 存储路径（相对）
    pub storage_path: String,
    /// 公开访问 URL
    pub public_url: String,
    /// 文件大小（bytes）
    pub size_bytes: i64,
}

/// 纯同步函数——必须在 `spawn_blocking` 内调用
/// 返回：(原图宽, 原图高, 缩略图列表)
pub fn generate_thumbnails(
    source_path: &Path,
    output_dir: &Path,
    media_id: &str,
    config: &ThumbnailGenerationConfig,
) -> AppResult<(u32, u32, Vec<ThumbnailInfo>)> {
    // 先只读尺寸（不完整解码，不占内存），超过阈值则跳过缩略图生成
    let reader = ImageReader::open(source_path)
        .map_err(|e| anyhow::anyhow!("Failed to open image: {}", e))?;
    let (orig_w, orig_h) = reader.into_dimensions()
        .map_err(|e| anyhow::anyhow!("Failed to read image dimensions: {}", e))?;

    // 基于 RGBA 解码后的内存估算（4 bytes per pixel）判断是否跳过
    let estimated_memory_bytes = orig_w as u64 * orig_h as u64 * 4;
    if estimated_memory_bytes > MAX_SOURCE_IMAGE_MEMORY_BYTES_FOR_THUMBNAIL_GENERATION {
        // 返回原始尺寸但空缩略图列表——不崩溃，只是不生成
        return Ok((orig_w, orig_h, Vec::new()));
    }

    // 然后完整解码（第二次打开文件走 OS 文件缓存，小图片上可接受）
    let img = image::open(source_path).map_err(|e| {
        anyhow::anyhow!("Failed to open image: {}", e)
    })?;

    let mut thumbnails = Vec::new();
    for &target_width in &config.widths {
        // 不放大：源图宽度 <= 目标宽度时跳过
        if target_width >= orig_w {
            continue;
        }
        let ratio = target_width as f64 / orig_w as f64;
        let target_height = (orig_h as f64 * ratio) as u32;

        let resized = img.resize_exact(target_width, target_height, FilterType::Lanczos3);

        let size_label = format!("{}w", target_width);
        let filename = format!("{}_thumb_{}.webp", media_id, size_label);
        let output_path = output_dir.join(&filename);

        // 编码为 WebP（VP8L 无损，image 0.25 不支持 quality 参数）
        let webp_bytes = {
            let mut cursor = std::io::Cursor::new(Vec::new());
            resized
                .write_to(&mut cursor, image::ImageFormat::WebP)
                .map_err(|e| anyhow::anyhow!("Failed to encode WebP: {}", e))?;
            cursor.into_inner()
        };

        let size_bytes = webp_bytes.len() as i64;
        std::fs::write(&output_path, &webp_bytes)?;

        // 读取缩略图实际尺寸（用于验证和返回精确信息）
        let thumb_img = image::open(&output_path).unwrap_or_else(|_| {
            image::DynamicImage::new_rgba8(target_width, target_height)
        });
        let actual_width = thumb_img.width();
        let actual_height = thumb_img.height();

        // 构建 storage_path（相对于 uploads 目录）
        let storage_path = format!("thumb/{}", filename);
        let public_url = format!("/uploads/thumb/{}", filename);

        thumbnails.push(ThumbnailInfo {
            size_label,
            width: actual_width,
            height: actual_height,
            storage_path,
            public_url,
            size_bytes,
        });
    }

    Ok((orig_w, orig_h, thumbnails))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建一个简单的测试 PNG（红蓝渐变，width x height 像素）
    fn create_test_png(width: u32, height: u32) -> Vec<u8> {
        use image::{ImageBuffer, Rgb};
        let mut img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::new(width, height);
        for (x, y, pixel) in img.enumerate_pixels_mut() {
            let r = (x as f32 / width as f32 * 255.0) as u8;
            let b = (y as f32 / height as f32 * 255.0) as u8;
            *pixel = Rgb([r, 0, b]);
        }
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        buf
    }

    #[test]
    fn test_generate_thumbnail_reduces_width_to_target() {
        let png_data = create_test_png(100, 100);
        let temp_dir = std::env::temp_dir();
        let source_path = temp_dir.join("test_source.png");
        std::fs::write(&source_path, &png_data).unwrap();

        let output_dir = temp_dir.clone();
        let config = ThumbnailGenerationConfig {
            widths: vec![50],
            keep_original: true,
        };

        let (orig_w, orig_h, thumbs) = generate_thumbnails(
            &source_path, &output_dir, "test-media-id", &config,
        )
        .unwrap();

        // 验证原图尺寸
        assert_eq!(orig_w, 100);
        assert_eq!(orig_h, 100);

        // 验证缩略图
        assert_eq!(thumbs.len(), 1);
        assert_eq!(thumbs[0].size_label, "50w");
        assert_eq!(thumbs[0].width, 50);
        assert!(thumbs[0].height <= 50); // 等比缩放
        assert!(thumbs[0].size_bytes > 0);

        // 验证文件存在且是 WebP
        let thumb_path = output_dir.join("test-media-id_thumb_50w.webp");
        assert!(thumb_path.exists());
        let thumb_bytes = std::fs::read(&thumb_path).unwrap();
        assert!(&thumb_bytes[0..4] == b"RIFF"); // WebP magic bytes

        // 验证缩略图可以重新打开
        let thumb_img = image::open(&thumb_path).unwrap();
        assert_eq!(thumb_img.width(), 50);

        // 清理
        std::fs::remove_file(&source_path).ok();
        std::fs::remove_file(&thumb_path).ok();
    }

    #[test]
    fn test_skip_upscaling_when_source_smaller_than_target() {
        let png_data = create_test_png(50, 50);
        let temp_dir = std::env::temp_dir();
        let source_path = temp_dir.join("test_small.png");
        std::fs::write(&source_path, &png_data).unwrap();

        let output_dir = temp_dir.clone();
        let config = ThumbnailGenerationConfig {
            widths: vec![100], // 目标 > 原图
            keep_original: true,
        };

        let (_, _, thumbs) = generate_thumbnails(
            &source_path, &output_dir, "test-upscale", &config,
        )
        .unwrap();

        // 不应生成缩略图（不能放大）
        assert!(thumbs.is_empty());

        std::fs::remove_file(&source_path).ok();
    }

    #[test]
    fn test_multiple_thumbnail_sizes() {
        let png_data = create_test_png(400, 300);
        let temp_dir = std::env::temp_dir();
        let source_path = temp_dir.join("test_multi.png");
        std::fs::write(&source_path, &png_data).unwrap();

        let output_dir = temp_dir.clone();
        let config = ThumbnailGenerationConfig {
            widths: vec![100, 200],
            keep_original: true,
        };

        let (_, _, thumbs) = generate_thumbnails(
            &source_path, &output_dir, "test-multi", &config,
        )
        .unwrap();

        assert_eq!(thumbs.len(), 2);
        assert_eq!(thumbs[0].size_label, "100w");
        assert_eq!(thumbs[0].width, 100);
        assert_eq!(thumbs[1].size_label, "200w");
        assert_eq!(thumbs[1].width, 200);

        for thumb in &thumbs {
            let path = output_dir.join(format!(
                "test-multi_thumb_{}.webp",
                thumb.size_label
            ));
            assert!(path.exists());
            let img = image::open(&path).unwrap();
            assert_eq!(img.width(), thumb.width);
            std::fs::remove_file(&path).ok();
        }

        std::fs::remove_file(&source_path).ok();
    }

    /// 验证小图片不受内存上限阈值影响，正常生成缩略图
    /// 200×200×4 = 160KB，远小于 30MB 阈值，应正常生成
    #[test]
    fn test_small_image_passes_memory_threshold() {
        let png_data = create_test_png(200, 200);
        let temp_dir = std::env::temp_dir();
        let source_path = temp_dir.join("test_threshold_pass.png");
        std::fs::write(&source_path, &png_data).unwrap();

        let output_dir = temp_dir.clone();
        let config = ThumbnailGenerationConfig {
            widths: vec![100],
            keep_original: true,
        };

        let (orig_w, orig_h, thumbs) = generate_thumbnails(
            &source_path, &output_dir, "test-threshold", &config,
        )
        .unwrap();

        // 200×200×4 = 160KB，远小于 30MB 阈值，应正常生成
        assert_eq!(orig_w, 200);
        assert_eq!(orig_h, 200);
        assert_eq!(thumbs.len(), 1);
        assert_eq!(thumbs[0].width, 100);

        let thumb_path = output_dir.join("test-threshold_thumb_100w.webp");
        std::fs::remove_file(&source_path).ok();
        std::fs::remove_file(&thumb_path).ok();
    }

    /// 验证超大图片的内存估算超过阈值
    /// 4000×3000×4 = 48MB > 30MB 阈值，应被跳过
    #[test]
    fn test_image_above_memory_threshold_estimation() {
        // 不实际创建大图（耗内存），只验证阈值计算逻辑
        let estimated = 4000u64 * 3000 * 4; // 48_000_000 bytes
        assert!(
            estimated > MAX_SOURCE_IMAGE_MEMORY_BYTES_FOR_THUMBNAIL_GENERATION,
            "4000×3000 RGBA ({} bytes) should exceed the {} byte threshold",
            estimated,
            MAX_SOURCE_IMAGE_MEMORY_BYTES_FOR_THUMBNAIL_GENERATION
        );

        // 验证中等图片可以通过
        let medium_estimated = 2000u64 * 1500 * 4; // 12_000_000 bytes
        assert!(
            medium_estimated < MAX_SOURCE_IMAGE_MEMORY_BYTES_FOR_THUMBNAIL_GENERATION,
            "2000×1500 RGBA ({} bytes) should be below the {} byte threshold",
            medium_estimated,
            MAX_SOURCE_IMAGE_MEMORY_BYTES_FOR_THUMBNAIL_GENERATION
        );
    }
}
