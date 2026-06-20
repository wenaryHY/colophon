//! WebP 图片转换纯函数。
//! 无状态、无 side effect，可独立单元测试。
//!
//! 依赖:
//! - `image` crate: JPEG/PNG 解码 + 缩放
//! - `zenwebp` crate: WebP 编码 (RGBA8 → VP8 lossy)

use std::io::Cursor;
use std::path::Path;

use zenwebp::{EncodeRequest, LossyConfig, PixelLayout};

use crate::shared::error::{AppError, AppResult};

/// 跳过转换的条件
///
/// 返回 true 时调用方应跳过转换、保留原文件。
pub fn should_skip_conversion(mime_type: &str, file_size: u64) -> bool {
    // GIF 动图跳过
    if mime_type == "image/gif" {
        return true;
    }
    // SVG 矢量图跳过
    if mime_type == "image/svg+xml" {
        return true;
    }
    // 已经是 WebP 跳过
    if mime_type == "image/webp" {
        return true;
    }
    // 超过 20MB 跳过
    if file_size > 20 * 1024 * 1024 {
        return true;
    }
    // 只转换 JPEG 和 PNG
    if mime_type != "image/jpeg" && mime_type != "image/png" {
        return true;
    }
    false
}

/// 将图片字节数据转换为 WebP，如果原图超过 max_edge 则等比缩放。
///
/// 返回 WebP 字节数据。失败时返回 AppError。
pub fn convert_to_webp(input: &[u8], max_edge: u32, quality: f32) -> AppResult<Vec<u8>> {
    // Step 0: 格式预检 — 只读图片头获取尺寸（不解码像素），
    // 用于判断是否需要后续的 resize 步骤
    let needs_resize = {
        let reader = image::ImageReader::new(Cursor::new(input))
            .with_guessed_format()
            .map_err(|e| AppError::BadRequest(format!("图片格式检测失败: {}", e)))?;
        let (w, h) = reader
            .into_dimensions()
            .map_err(|e| AppError::BadRequest(format!("图片尺寸读取失败: {}", e)))?;
        w > max_edge || h > max_edge
    };

    // Step 1: 解码
    let img = image::load_from_memory(input)
        .map_err(|e| AppError::BadRequest(format!("图片解码失败: {}", e)))?;

    // Step 2: 条件缩放（CatmullRom kernel support 2.0，比 Lanczos3 3.0 快约 30%）
    let img = if needs_resize {
        let (w, h) = (img.width(), img.height());
        let scale = max_edge as f64 / (w.max(h) as f64);
        let nw = (w as f64 * scale) as u32;
        let nh = (h as f64 * scale) as u32;
        img.resize_exact(nw, nh, image::imageops::FilterType::CatmullRom)
    } else {
        img
    };

    let (nw, nh) = (img.width(), img.height());

    // Step 3: 根据实际 alpha 通道选择像素格式（RGB8 比 RGBA8 省 25% 内存和编码时间）
    let has_alpha = img.color().has_alpha();
    let config = LossyConfig::new()
        .with_quality(quality)
        .with_method(3); // method 3 ~15% 快于默认 4，质量损失微小

    let webp_bytes = if has_alpha {
        let rgba = img.into_rgba8();
        EncodeRequest::lossy(&config, rgba.as_raw(), PixelLayout::Rgba8, nw, nh).encode()
    } else {
        let rgb = img.into_rgb8();
        EncodeRequest::lossy(&config, rgb.as_raw(), PixelLayout::Rgb8, nw, nh).encode()
    }
    .map_err(|e| AppError::BadRequest(format!("WebP 编码失败: {}", e)))?;

    Ok(webp_bytes)
}

/// 原子写入：写临时文件 → 验证 magic bytes → rename → 返回
///
/// 如果任何一步失败，临时文件会被清理。
pub async fn atomic_write_webp(
    temp_path: &Path,
    final_path: &Path,
    data: &[u8],
) -> AppResult<()> {
    // 1. 写临时文件
    tokio::fs::write(temp_path, data)
        .await
        .map_err(|e| AppError::BadRequest(format!("临时文件写入失败: {}", e)))?;

    // 2. 验证 magic bytes（RIFF + WEBP）
    let verify = tokio::fs::read(temp_path)
        .await
        .map_err(|e| AppError::BadRequest(format!("临时文件验证读取失败: {}", e)))?;
    if verify.len() < 12 || &verify[0..4] != b"RIFF" || &verify[8..12] != b"WEBP" {
        let _ = tokio::fs::remove_file(temp_path).await;
        return Err(AppError::BadRequest(
            "WebP 文件验证失败（magic bytes 不匹配）".into(),
        ));
    }

    // 3. 原子 rename（失败时清理临时文件，避免磁盘垃圾）
    tokio::fs::rename(temp_path, final_path)
        .await
        .map_err(|e| {
            let _ = std::fs::remove_file(temp_path);
            AppError::BadRequest(format!("文件重命名失败: {}", e))
        })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_skip_gif() {
        assert!(should_skip_conversion("image/gif", 1024));
    }

    #[test]
    fn should_skip_svg() {
        assert!(should_skip_conversion("image/svg+xml", 1024));
    }

    #[test]
    fn should_skip_webp() {
        assert!(should_skip_conversion("image/webp", 1024));
    }

    #[test]
    fn should_skip_large_file() {
        assert!(should_skip_conversion("image/jpeg", 21 * 1024 * 1024));
    }

    #[test]
    fn should_not_skip_small_jpeg() {
        assert!(!should_skip_conversion("image/jpeg", 1024));
    }

    #[test]
    fn should_not_skip_small_png() {
        assert!(!should_skip_conversion("image/png", 5 * 1024 * 1024));
    }

    #[test]
    fn should_skip_unsupported_mime() {
        assert!(should_skip_conversion("video/mp4", 1024));
    }

    /// 构造最小合法 JPEG（通过 image crate 编码，保证有效性）
    #[test]
    fn convert_tiny_jpeg_to_webp() {
        // 用 image crate 构造 1x1 白色 JPEG
        let img = image::DynamicImage::new_rgb8(1, 1);
        let mut jpeg_bytes = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut jpeg_bytes), image::ImageFormat::Jpeg)
            .expect("failed to encode test JPEG");

        let result = convert_to_webp(&jpeg_bytes, 2048, 80.0);
        assert!(result.is_ok(), "conversion failed: {:?}", result.err());
        let webp = result.unwrap();

        // 验证 WebP magic bytes
        assert!(
            webp.len() >= 12 && &webp[0..4] == b"RIFF" && &webp[8..12] == b"WEBP",
            "output is not valid WebP"
        );
    }

    /// 输入过大图片触发缩放
    #[test]
    fn convert_with_downscale() {
        // 100x100 白色图片 — 超过 max_edge=50，应触发等比缩放到 50x50
        // JPEG 格式只支持 RGB，所以用 Rgb8 构造
        let img = image::DynamicImage::new_rgb8(100, 100);

        let mut jpeg_bytes = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut jpeg_bytes), image::ImageFormat::Jpeg)
            .expect("failed to encode test JPEG");

        let result = convert_to_webp(&jpeg_bytes, 50, 80.0);
        assert!(result.is_ok(), "conversion with resize failed: {:?}", result.err());
        let webp = result.unwrap();
        assert!(webp.len() >= 12 && &webp[0..4] == b"RIFF" && &webp[8..12] == b"WEBP");

        // 验证缩放后尺寸为 50x50
        let decoded = image::load_from_memory(&webp)
            .expect("decoded webp should be valid");
        assert_eq!(decoded.width(), 50);
        assert_eq!(decoded.height(), 50);
    }

    // -- atomic_write_webp 测试 --

    /// 生成最小合法 WebP 字节（lossy VP8, 16x16 px）
    /// RIFF + WEBP + VP8 chunk，仅用于通过 magic bytes 验证
    fn make_minimal_webp_bytes() -> Vec<u8> {
        vec![
            0x52, 0x49, 0x46, 0x46, // "RIFF"
            0x1a, 0x00, 0x00, 0x00, // file size - 8 (26 bytes, LE)
            0x57, 0x45, 0x42, 0x50, // "WEBP"
            0x56, 0x50, 0x38, 0x20, // "VP8 "
            0x0e, 0x00, 0x00, 0x00, // chunk size (14 bytes, LE)
            0x10, 0x00, // width (16, LE)
            0x00,       // scale x
            0x10, 0x00, // height (16, LE)
            0x00,       // scale y
            0x2f, 0x41, 0x54, 0x49, // key frame data
            0x54, 0x41, 0x4c, 0x44,
        ]
    }

    /// 测试原子写入的完整流程：写 tmp -> 验证 magic bytes -> rename
    #[tokio::test]
    async fn test_atomic_write_webp_success() {
        let dir = tempfile::tempdir().unwrap();
        let temp_path = dir.path().join("test.jpg.webp.tmp");
        let final_path = dir.path().join("test.jpg");

        let webp_data = make_minimal_webp_bytes();

        atomic_write_webp(&temp_path, &final_path, &webp_data)
            .await
            .unwrap();

        // 验证：tmp 文件已消失（rename 后），final 文件存在且内容正确
        assert!(!temp_path.exists());
        assert!(final_path.exists());
        let content = tokio::fs::read(&final_path).await.unwrap();
        assert_eq!(content, webp_data);
    }

    /// 测试 magic bytes 验证失败时原子写入回滚
    #[tokio::test]
    async fn test_atomic_write_webp_invalid_magic_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let temp_path = dir.path().join("test.jpg.webp.tmp");
        let final_path = dir.path().join("test.jpg");

        let invalid_data = b"not a webp file";

        let result = atomic_write_webp(&temp_path, &final_path, invalid_data).await;

        // 验证：写入失败，tmp 文件被清理，final 文件不存在
        assert!(result.is_err());
        assert!(!temp_path.exists());
        assert!(!final_path.exists());
    }

    // -- PNG 转换测试 --

    /// 测试 PNG（带 alpha 通道）→ WebP 转换走 Rgba8 路径
    #[test]
    fn test_convert_png_with_alpha_to_webp() {
        // 构造一个 64x64 的 RGBA PNG（含半透明像素）
        let png_bytes = make_test_png(64, 64, true);
        let result = convert_to_webp(&png_bytes, 2048, 75.0);
        assert!(result.is_ok());
        let webp = result.unwrap();
        assert!(!webp.is_empty());
        // 验证 magic bytes
        assert_eq!(&webp[0..4], b"RIFF");
        assert_eq!(&webp[8..12], b"WEBP");
    }

    /// 测试 PNG（不带 alpha 通道）→ WebP 转换走 Rgb8 路径
    #[test]
    fn test_convert_png_without_alpha_to_webp() {
        let png_bytes = make_test_png(64, 64, false);
        let result = convert_to_webp(&png_bytes, 2048, 75.0);
        assert!(result.is_ok());
        let webp = result.unwrap();
        assert!(!webp.is_empty());
        assert_eq!(&webp[0..4], b"RIFF");
        assert_eq!(&webp[8..12], b"WEBP");
    }

    /// 辅助：用 image crate 生成测试 PNG（RGB 或 RGBA）
    fn make_test_png(width: u32, height: u32, with_alpha: bool) -> Vec<u8> {
        use image::{RgbaImage, RgbImage};
        let mut buf = Vec::new();
        if with_alpha {
            let mut img = RgbaImage::new(width, height);
            for (x, y, pixel) in img.enumerate_pixels_mut() {
                // 棋盘格 + 半透明
                let v = if (x + y) % 2 == 0 { 255u8 } else { 128u8 };
                *pixel = image::Rgba([v, v, v, 200]);
            }
            img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png).unwrap();
        } else {
            let mut img = RgbImage::new(width, height);
            for (x, y, pixel) in img.enumerate_pixels_mut() {
                let v = if (x + y) % 2 == 0 { 255u8 } else { 128u8 };
                *pixel = image::Rgb([v, v, v]);
            }
            img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png).unwrap();
        }
        buf
    }
}
