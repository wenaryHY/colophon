//! Schema-as-Code 数据结构定义。
//!
//! 包含 TOML 解析结构（ColophonSchema）和模板上下文结构（TemplateContext）。

use serde::{Deserialize, Serialize};

// ── TOML 解析结构 ──────────────────────────────────────────────────────────

/// 顶层 Schema 定义，对应 `schemas/*.toml` 文件。
#[derive(Debug, Deserialize)]
pub struct ColophonSchema {
    pub collection: CollectionDef,
    #[serde(default)]
    pub features: FeaturesDef,
    pub fields: Vec<FieldDef>,
}

/// 集合元信息。
#[derive(Debug, Deserialize)]
pub struct CollectionDef {
    /// Rust 侧模型名，如 "Category"。
    pub name: String,
    /// SQLite 表名，如 "categories"。
    pub table: String,
    /// 用户可读的显示名，如 "分类"。
    pub display_name: Option<String>,
}

/// 功能开关，控制自动注入哪些系统字段。
#[derive(Debug, Deserialize, Serialize, Default)]
pub struct FeaturesDef {
    /// 是否注入 `deleted_at` 字段。
    #[serde(default)]
    pub soft_delete: bool,
    /// 是否注入 `created_at` 和 `updated_at` 字段。
    #[serde(default)]
    pub timestamps: bool,
    /// 是否注入 `sort_order` 字段。
    #[serde(default)]
    pub sort_order: bool,
}

/// 字段定义，TOML 中的 `[[fields]]` 条目。
///
/// `is_updatable`、`is_auto_generated`、`rust_type`、`sqlite_type`
/// 在 schema 文件解析后由 Context Builder 加工填充，
/// 在锁文件解析后直接从文件中恢复。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDef {
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: String,
    #[serde(default = "default_true")]
    pub required: bool,
    #[serde(default)]
    pub unique: bool,
    #[serde(default)]
    pub computed: bool,
    pub references: Option<String>,

    // ── 加工生成的内部状态 ──
    // schema 文件中省略时默认 false，锁文件中会序列化/反序列化
    #[serde(default)]
    pub is_updatable: bool,
    #[serde(default)]
    pub is_auto_generated: bool,
    #[serde(default)]
    pub rust_type: String,
    #[serde(default)]
    pub sqlite_type: String,
}

fn default_true() -> bool {
    true
}

// ── 模板上下文 ─────────────────────────────────────────────────────────────

/// 供模板引擎使用的完整上下文，由 Context Builder 从 ColophonSchema 加工而来。
#[derive(Debug, Serialize)]
pub struct TemplateContext {
    /// Rust 模型名，如 "Category"。
    pub model_name: String,
    /// SQLite 表名，如 "categories"。
    pub table_name: String,
    /// 用户可读显示名，如 "分类"。
    pub display_name: String,
    /// 功能开关，供模板条件渲染使用。
    pub features: FeaturesDef,
    /// 所有字段（含自动注入的 id、timestamps、soft_delete、sort_order）。
    pub fields: Vec<FieldDef>,
    /// CreateDTO 字段：过滤掉 computed 字段，每项附带 create_type。
    pub create_fields: Vec<TemplateField>,
    /// UpdateDTO 字段：过滤掉 id 字段，每项附带 update_type。
    pub update_fields: Vec<TemplateField>,
    /// INSERT 语句字段：过滤掉 auto_generated 字段。
    pub insert_fields: Vec<TemplateField>,
    /// SELECT 列列表，逗号分隔，如 "id, name, slug, created_at"。
    pub select_columns: String,
    /// INSERT 列列表，逗号分隔（不含 id），如 "name, slug"。
    pub insert_columns: String,
    /// INSERT 占位符，如 "?, ?, ?"。
    pub insert_placeholders: String,
    /// UPDATE SET 子句，如 "name = ?, slug = ?"。
    pub update_set_clause: String,
}

/// 带模板渲染类型信息的字段，从 FieldDef 转换而来。
///
/// 使用组合模式：内部包含 FieldDef，额外添加 create_type / update_type / param_type。
/// 通过 Deref 可以直接访问 FieldDef 的所有字段。
#[derive(Debug, Clone, Serialize)]
pub struct TemplateField {
    /// 内部 FieldDef，包含所有基础字段。
    #[serde(flatten)]
    pub field: FieldDef,
    /// Create DTO 中的类型：required 字段为 `T`，非 required 为 `Option<T>`。
    pub create_type: String,
    /// Update DTO 中的类型：始终为 `Option<T>`。
    pub update_type: String,
    /// Repository 函数参数类型：`String` → `&str`，`i64` → `i64`，`bool` → `bool`。
    /// 用于 insert/update 函数签名。
    pub param_type: String,
    /// Repository 函数可选参数类型：如 `Option<&str>`、`Option<i64>`。
    pub opt_param_type: String,
}

impl std::ops::Deref for TemplateField {
    type Target = FieldDef;

    fn deref(&self) -> &Self::Target {
        &self.field
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fix_l3_template_field_uses_field_def_ref() {
        let field_def = FieldDef {
            name: "title".into(),
            field_type: "text".into(),
            required: true,
            unique: false,
            computed: false,
            references: None,
            is_updatable: true,
            is_auto_generated: false,
            rust_type: "String".into(),
            sqlite_type: "TEXT NOT NULL".into(),
        };

        let template_field = TemplateField {
            field: field_def.clone(),
            create_type: "String".into(),
            update_type: "Option<String>".into(),
            param_type: "&str".into(),
            opt_param_type: "Option<&str>".into(),
        };

        // 验证通过 Deref 可以访问 FieldDef 的字段
        assert_eq!(template_field.name, "title");
        assert_eq!(template_field.field_type, "text");
        assert!(template_field.required);
        assert_eq!(template_field.rust_type, "String");
        assert_eq!(template_field.sqlite_type, "TEXT NOT NULL");

        // 验证额外字段
        assert_eq!(template_field.create_type, "String");
        assert_eq!(template_field.update_type, "Option<String>");

        // 验证 field 引用正确
        assert_eq!(template_field.field.name, field_def.name);
    }
}
