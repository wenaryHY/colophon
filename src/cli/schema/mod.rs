//! Schema-as-Code CLI 模块。
//!
//! 职责：
//! - 解析 `schemas/*.toml` 为 ColophonSchema
//! - 通过 Context Builder 加工为 TemplateContext（供模板引擎使用）

pub mod context;
pub mod diff;
pub mod generate;
pub mod types;

use std::path::Path;

use anyhow::{Context, Result};

use self::context::build_context;
use self::types::{ColophonSchema, TemplateContext};

/// 从单个 TOML 文件解析并构建 TemplateContext。
pub fn parse_schema_file(path: &Path) -> Result<TemplateContext> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("无法读取 Schema 文件: {}", path.display()))?;

    let schema: ColophonSchema =
        toml::from_str(&content).with_context(|| format!("TOML 解析失败: {}", path.display()))?;

    build_context(schema).with_context(|| format!("Schema 校验失败: {}", path.display()))
}

/// 扫描目录下所有 `*.toml` 文件，解析并构建 TemplateContext 列表。
pub fn parse_schema_dir(dir: &Path) -> Result<Vec<TemplateContext>> {
    if !dir.is_dir() {
        anyhow::bail!("Schema 目录不存在: {}", dir.display());
    }

    let mut contexts = Vec::new();

    let entries: Vec<_> = std::fs::read_dir(dir)
        .with_context(|| format!("无法读取目录: {}", dir.display()))?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "toml"))
        .collect();

    for entry in entries {
        let path = entry.path();
        let ctx = parse_schema_file(&path)?;
        contexts.push(ctx);
    }

    Ok(contexts)
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::io::Write;

    /// 创建临时 TOML 文件并验证端到端解析。
    #[test]
    fn parse_toml_file_end_to_end() {
        let toml_content = r#"
[collection]
name = "Category"
table = "categories"
display_name = "分类"

[features]
soft_delete = true
timestamps = true
sort_order = true

[[fields]]
name = "name"
type = "text"
required = true
unique = true

[[fields]]
name = "slug"
type = "text"
required = true
unique = true

[[fields]]
name = "description"
type = "richtext"
required = false

[[fields]]
name = "parent_id"
type = "relation"
required = false
"#;

        let mut tmpfile = tempfile::NamedTempFile::new().expect("创建临时文件失败");
        tmpfile
            .write_all(toml_content.as_bytes())
            .expect("写入临时文件失败");

        let ctx = parse_schema_file(tmpfile.path()).expect("解析失败");

        // 验证模型信息
        assert_eq!(ctx.model_name, "Category");
        assert_eq!(ctx.table_name, "categories");
        assert_eq!(ctx.display_name, "分类");

        // 验证字段数量：id + 4个用户字段(name, slug, description, parent_id) + created_at + updated_at + deleted_at + sort_order = 9
        assert_eq!(ctx.fields.len(), 9);

        // 验证 id 字段
        let id = ctx.fields.first().unwrap();
        assert_eq!(id.name, "id");
        assert!(id.is_auto_generated);

        // 验证 name 字段
        let name = ctx.fields.iter().find(|f| f.name == "name").unwrap();
        assert_eq!(name.rust_type, "String");
        assert_eq!(name.sqlite_type, "TEXT NOT NULL");
        assert!(name.unique);
        assert!(name.required);

        // 验证 parent_id (relation)
        let parent = ctx.fields.iter().find(|f| f.name == "parent_id").unwrap();
        assert_eq!(parent.rust_type, "Option<String>");
        assert_eq!(parent.sqlite_type, "TEXT");

        // 验证 description (richtext, optional)
        let desc = ctx.fields.iter().find(|f| f.name == "description").unwrap();
        assert_eq!(desc.rust_type, "String");
        assert!(!desc.required);

        // 验证 create_fields 不含 computed 字段
        assert!(ctx.create_fields.iter().all(|f| !f.computed));

        // 验证 update_fields 排除了 id
        assert!(ctx.update_fields.iter().all(|f| f.name != "id"));
        assert!(ctx.update_fields.iter().any(|f| f.name == "name"));

        // 验证 insert_fields 排除了 auto_generated
        assert!(ctx.insert_fields.iter().all(|f| !f.is_auto_generated));
        assert!(ctx.insert_fields.iter().any(|f| f.name == "name"));

        // 验证 select_columns 包含所有字段名
        assert!(ctx.select_columns.contains("id"));
        assert!(ctx.select_columns.contains("name"));
        assert!(ctx.select_columns.contains("created_at"));
        assert!(ctx.select_columns.contains("deleted_at"));
        assert!(ctx.select_columns.contains("sort_order"));
    }

    #[test]
    fn parse_invalid_toml_returns_error() {
        let mut tmpfile = tempfile::NamedTempFile::new().expect("创建临时文件失败");
        tmpfile.write_all(b"not valid toml [[[").expect("写入失败");

        let result = parse_schema_file(tmpfile.path());
        assert!(result.is_err());
    }

    #[test]
    fn parse_schema_dir_reads_all_toml_files() {
        let dir = tempfile::tempdir().expect("创建临时目录失败");

        // 写入两个 toml 文件
        let toml_a = r#"
[collection]
name = "Tag"
table = "tags"

[[fields]]
name = "name"
type = "text"
"#;
        let toml_b = r#"
[collection]
name = "Media"
table = "media"

[[fields]]
name = "path"
type = "text"
"#;

        std::fs::write(dir.path().join("tag.toml"), toml_a).expect("写入失败");
        std::fs::write(dir.path().join("media.toml"), toml_b).expect("写入失败");
        // 放一个非 toml 文件，应被忽略
        std::fs::write(dir.path().join("readme.txt"), "ignore me").expect("写入失败");

        let contexts = parse_schema_dir(dir.path()).expect("扫描目录失败");
        assert_eq!(contexts.len(), 2);

        let names: Vec<&str> = contexts.iter().map(|c| c.model_name.as_str()).collect();
        assert!(names.contains(&"Tag"));
        assert!(names.contains(&"Media"));
    }

    #[test]
    fn parse_schema_dir_nonexistent_returns_error() {
        let result = parse_schema_dir(Path::new("/nonexistent/path"));
        assert!(result.is_err());
    }

    /// 解析实际的 tag.toml 文件，验证字段类型正确填充。
    #[test]
    fn parse_actual_tag_toml_has_correct_types() {
        let project_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let tag_toml = project_root.join("schemas").join("tag.toml");

        if !tag_toml.exists() {
            // 如果 tag.toml 不存在，跳过测试
            return;
        }

        let ctx = parse_schema_file(&tag_toml).expect("解析 tag.toml 失败");

        assert_eq!(ctx.model_name, "Tag");
        assert_eq!(ctx.table_name, "tags");

        // 验证 id 字段类型正确
        let id = ctx.fields.iter().find(|f| f.name == "id").unwrap();
        assert_eq!(id.rust_type, "String");
        assert_eq!(id.sqlite_type, "TEXT PRIMARY KEY NOT NULL");
        assert!(id.is_auto_generated);

        // 验证 name 字段类型正确
        let name = ctx.fields.iter().find(|f| f.name == "name").unwrap();
        assert_eq!(name.rust_type, "String");
        assert_eq!(name.sqlite_type, "TEXT NOT NULL");
        assert!(name.is_updatable);

        // 验证 slug 字段类型正确
        let slug = ctx.fields.iter().find(|f| f.name == "slug").unwrap();
        assert_eq!(slug.rust_type, "String");
        assert_eq!(slug.sqlite_type, "TEXT NOT NULL");
    }
}
