//! `colophon export` 命令：将 SQLite 数据库内容导出为 JSON 文件，供静态站点生成器（Astro/Next.js）使用。

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

/// 导出入口：连接数据库 → 查询各表 → 写 JSON → 复制媒体文件。
pub async fn run(database: PathBuf, output_dir: PathBuf, upload_dir: PathBuf) -> Result<()> {
    // 解析上传目录的绝对路径（导出前确保文件复制源目录存在）
    let upload_dir_absolute = resolve_path(&upload_dir)?;

    std::fs::create_dir_all(&output_dir).context("无法创建输出目录")?;
    let media_output_dir = output_dir.join("media");
    std::fs::create_dir_all(&media_output_dir).context("无法创建媒体输出目录")?;

    let pool = open_database(&database).await?;

    eprintln!("[colophon export] 开始导出到 {}", output_dir.display());

    // 按依赖关系最小化的顺序导出：基础数据 → 关联数据 → 媒体文件
    export_settings(&pool, &output_dir).await?;
    export_tags(&pool, &output_dir).await?;
    export_categories(&pool, &output_dir).await?;
    export_posts(&pool, &output_dir).await?;
    export_pages(&pool, &output_dir).await?;
    export_post_tags(&pool, &output_dir).await?;
    export_media_metadata(&pool, &output_dir).await?;
    export_media_files(&pool, &output_dir, &upload_dir_absolute).await?;

    eprintln!("[colophon export] 导出完成");
    Ok(())
}

// ── 数据库连接 ────────────────────────────────────────────────────────────

async fn open_database(database: &Path) -> Result<SqlitePool> {
    let abs_path = resolve_path(database)?;
    let database_url = format!("sqlite:{}?mode=ro", abs_path.display());
    let pool = SqlitePoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await
        .context("无法连接数据库")?;
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await?;
    Ok(pool)
}

fn resolve_path(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

// ── JSON 查询辅助 ─────────────────────────────────────────────────────────
// 使用 SQLite 的 json_object() 直接在数据库层生成 JSON 字符串，
// 避免 Rust 端枚举/bool 等类型映射问题，同时保持字段顺序可控。

async fn query_json_rows(pool: &SqlitePool, sql: &str) -> Result<Vec<String>> {
    let rows: Vec<(String,)> = sqlx::query_as(sql)
        .fetch_all(pool)
        .await
        .context("数据库查询失败")?;
    Ok(rows.into_iter().map(|(json,)| json).collect())
}

fn write_json_array(path: &Path, items: &[String]) -> Result<()> {
    // 手动拼接 JSON 数组以实现 pretty-print，避免两次解析（str → Value → str）
    let mut buf = String::from("[\n");
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            buf.push_str(",\n");
        }
        // 将紧凑的单行 JSON 格式化：先解析再 pretty-print
        let value: serde_json::Value = serde_json::from_str(item).context("JSON 解析失败")?;
        let pretty = serde_json::to_string_pretty(&value)?;
        // 缩进两空格
        for line in pretty.lines() {
            buf.push_str("  ");
            buf.push_str(line);
            buf.push('\n');
        }
        // 移除末尾多余的换行（由下一个 item 的逗号或最后的换行替代）
        buf.pop();
    }
    buf.push_str("\n]\n");
    std::fs::write(path, &buf).context("写入 JSON 文件失败")?;
    Ok(())
}

// ── 各实体导出函数 ────────────────────────────────────────────────────────

/// 导出已发布文章（posts），包含作者、分类信息。
async fn export_posts(pool: &SqlitePool, output_dir: &Path) -> Result<()> {
    eprintln!("  导出 posts...");
    let sql = r#"
        SELECT json_object(
            'id',             p.id,
            'title',          p.title,
            'slug',           p.slug,
            'excerpt',        p.excerpt,
            'content_md',     p.content_md,
            'content_html',   p.content_html,
            'cover_media_id', p.cover_media_id,
            'status',         p.status,
            'visibility',     p.visibility,
            'content_type',   p.content_type,
            'custom_html_path', p.custom_html_path,
            'page_render_mode', p.page_render_mode,
            'allow_comment',  p.allow_comment,
            'pinned',         p.pinned,
            'author_id',      p.author_id,
            'author_display_name', u.display_name,
            'category_id',    p.category_id,
            'category_name',  c.name,
            'category_slug',  c.slug,
            'published_at',   p.published_at,
            'created_at',     p.created_at,
            'updated_at',     p.updated_at
        )
        FROM posts p
        JOIN users u ON u.id = p.author_id
        LEFT JOIN categories c ON c.id = p.category_id
        WHERE p.status = 'published'
          AND p.content_type = 'post'
          AND p.deleted_at IS NULL
        ORDER BY p.published_at DESC
    "#;
    let rows = query_json_rows(pool, sql).await?;
    let path = output_dir.join("posts.json");
    write_json_array(&path, &rows)?;
    eprintln!("    -> {} 篇文章", rows.len());
    Ok(())
}

/// 导出页面（content_type = 'page'），不区分发布状态（未发布的页面也导出供预览）。
async fn export_pages(pool: &SqlitePool, output_dir: &Path) -> Result<()> {
    eprintln!("  导出 pages...");
    let sql = r#"
        SELECT json_object(
            'id',               p.id,
            'title',            p.title,
            'slug',             p.slug,
            'excerpt',          p.excerpt,
            'content_md',       p.content_md,
            'content_html',     p.content_html,
            'cover_media_id',   p.cover_media_id,
            'status',           p.status,
            'visibility',       p.visibility,
            'content_type',     p.content_type,
            'custom_html_path', p.custom_html_path,
            'page_render_mode', p.page_render_mode,
            'allow_comment',    p.allow_comment,
            'pinned',           p.pinned,
            'author_id',        p.author_id,
            'published_at',     p.published_at,
            'created_at',       p.created_at,
            'updated_at',       p.updated_at
        )
        FROM posts p
        WHERE p.content_type = 'page'
          AND p.deleted_at IS NULL
        ORDER BY p.title ASC
    "#;
    let rows = query_json_rows(pool, sql).await?;
    let path = output_dir.join("pages.json");
    write_json_array(&path, &rows)?;
    eprintln!("    -> {} 个页面", rows.len());
    Ok(())
}

/// 导出标签。
async fn export_tags(pool: &SqlitePool, output_dir: &Path) -> Result<()> {
    eprintln!("  导出 tags...");
    let sql = r#"
        SELECT json_object(
            'id',         t.id,
            'name',       t.name,
            'slug',       t.slug,
            'created_at', t.created_at,
            'updated_at', t.updated_at
        )
        FROM tags t
        WHERE t.deleted_at IS NULL
        ORDER BY t.name ASC
    "#;
    let rows = query_json_rows(pool, sql).await?;
    let path = output_dir.join("tags.json");
    write_json_array(&path, &rows)?;
    eprintln!("    -> {} 个标签", rows.len());
    Ok(())
}

/// 导出分类。
async fn export_categories(pool: &SqlitePool, output_dir: &Path) -> Result<()> {
    eprintln!("  导出 categories...");
    let sql = r#"
        SELECT json_object(
            'id',          c.id,
            'name',        c.name,
            'slug',        c.slug,
            'description', c.description,
            'parent_id',   c.parent_id,
            'sort_order',  c.sort_order,
            'created_at',  c.created_at,
            'updated_at',  c.updated_at
        )
        FROM categories c
        WHERE c.deleted_at IS NULL
        ORDER BY c.sort_order ASC, c.name ASC
    "#;
    let rows = query_json_rows(pool, sql).await?;
    let path = output_dir.join("categories.json");
    write_json_array(&path, &rows)?;
    eprintln!("    -> {} 个分类", rows.len());
    Ok(())
}

/// 导出文章—标签多对多关系。
async fn export_post_tags(pool: &SqlitePool, output_dir: &Path) -> Result<()> {
    eprintln!("  导出 post_tags...");
    let sql = r#"
        SELECT json_object(
            'post_id',  pt.post_id,
            'tag_id',   pt.tag_id,
            'tag_name', t.name,
            'tag_slug', t.slug
        )
        FROM post_tags pt
        JOIN tags t ON t.id = pt.tag_id
        WHERE t.deleted_at IS NULL
        ORDER BY pt.post_id, t.name ASC
    "#;
    let rows = query_json_rows(pool, sql).await?;
    let path = output_dir.join("post_tags.json");
    write_json_array(&path, &rows)?;
    eprintln!("    -> {} 条关联", rows.len());
    Ok(())
}

/// 导出站点设置（key-value 对）。
async fn export_settings(pool: &SqlitePool, output_dir: &Path) -> Result<()> {
    eprintln!("  导出 settings...");
    let sql = r#"
        SELECT json_object(
            'key',        s.key,
            'value',      s.value,
            'updated_at', s.updated_at
        )
        FROM settings s
        ORDER BY s.key ASC
    "#;
    let rows = query_json_rows(pool, sql).await?;
    let path = output_dir.join("settings.json");
    write_json_array(&path, &rows)?;
    eprintln!("    -> {} 项设置", rows.len());
    Ok(())
}

/// 导出媒体元数据（不复制文件本身）。
async fn export_media_metadata(pool: &SqlitePool, output_dir: &Path) -> Result<()> {
    eprintln!("  导出 media metadata...");
    let sql = r#"
        SELECT json_object(
            'id',           m.id,
            'kind',         m.kind,
            'mime_type',    m.mime_type,
            'original_name', m.original_name,
            'stored_name',  m.stored_name,
            'storage_path', m.storage_path,
            'public_url',   m.public_url,
            'size_bytes',   m.size_bytes,
            'width',        m.width,
            'height',       m.height,
            'duration_seconds', m.duration_seconds,
            'alt_text',     m.alt_text,
            'category',     m.category,
            'created_at',   m.created_at
        )
        FROM media m
        WHERE m.deleted_at IS NULL
        ORDER BY m.created_at DESC
    "#;
    let rows = query_json_rows(pool, sql).await?;
    let path = output_dir.join("media.json");
    write_json_array(&path, &rows)?;
    eprintln!("    -> {} 个媒体文件", rows.len());
    Ok(())
}

/// 将上传目录中的媒体文件复制到输出目录的 media/ 子目录。
async fn export_media_files(pool: &SqlitePool, output_dir: &Path, upload_dir: &Path) -> Result<()> {
    // 查询所有未删除媒体的 storage_path
    let rows: Vec<(String,)> =
        sqlx::query_as("SELECT storage_path FROM media WHERE deleted_at IS NULL")
            .fetch_all(pool)
            .await
            .context("查询媒体文件路径失败")?;

    let media_output_dir = output_dir.join("media");
    let mut copied = 0usize;

    for (storage_path,) in &rows {
        let source = upload_dir.join(storage_path);
        let dest = media_output_dir.join(storage_path);

        if !source.exists() {
            eprintln!("    警告: 媒体文件不存在: {}", source.display());
            continue;
        }

        // 确保目标父目录存在
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("无法创建目录: {}", parent.display()))?;
        }

        std::fs::copy(&source, &dest)
            .with_context(|| format!("复制文件失败: {} -> {}", source.display(), dest.display()))?;
        copied += 1;
    }

    eprintln!("    -> 复制了 {} 个媒体文件", copied);
    Ok(())
}
