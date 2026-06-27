//! Context Builder：将 ColophonSchema 加工为 TemplateContext。
//!
//! 职责：
//! 1. 自动注入 id 字段
//! 2. 展开 features（timestamps / soft_delete / sort_order）
//! 3. 计算类型映射（field_type → rust_type / sqlite_type）
//! 4. 标记 is_updatable / is_auto_generated
//! 5. 生成各类视图字段列表

use super::types::{ColophonSchema, FeaturesDef, FieldDef, TemplateContext, TemplateField};
use anyhow::Result;
use tracing;

// ── 命名转换 ───────────────────────────────────────────────────────────────

/// 将 snake_case 转换为 PascalCase。
///
/// 用于将 TOML 中的 `name = "label"` 转换为 Rust struct 名 `Label`。
/// 如果输入已经是 PascalCase（如 "Category"、"BlogPost"），原样返回。
fn to_pascal_case(s: &str) -> String {
    // 如果没有下划线且首字母已大写，视为已 PascalCase，原样返回
    if !s.contains('_') {
        if let Some(first) = s.chars().next() {
            if first.is_uppercase() {
                return s.to_string();
            }
        }
    }

    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => {
                    let upper: String = first.to_uppercase().collect();
                    upper + &chars.as_str().to_lowercase()
                }
                None => String::new(),
            }
        })
        .collect()
}

// ── 类型映射 ───────────────────────────────────────────────────────────────

/// 根据 field_type 返回 (rust_type, sqlite_type)。
///
/// relation 类型的 rust_type 受 required 影响，需要在外层单独处理。
fn map_type(field_type: &str, required: bool) -> (&'static str, &'static str) {
    match field_type {
        "text" => ("String", "TEXT NOT NULL"),
        "richtext" => ("String", "TEXT NOT NULL"),
        "boolean" => ("bool", "INTEGER NOT NULL DEFAULT 0"),
        "integer" => ("i64", "INTEGER NOT NULL DEFAULT 0"),
        "timestamp" => (
            "String",
            "TEXT NOT NULL DEFAULT (datetime('now'))",
        ),
        "relation" => {
            // relation 始终是 Option，即使 required = true
            if required {
                ("Option<String>", "TEXT")
            } else {
                ("Option<String>", "TEXT")
            }
        }
        // 未知类型保守处理为 String
        unknown => {
            tracing::warn!(
                field_type = unknown,
                "未知的字段类型，默认映射为 String。支持的类型: text, richtext, boolean, integer, timestamp, relation"
            );
            ("String", "TEXT NOT NULL")
        }
    }
}

// ── 系统字段工厂 ───────────────────────────────────────────────────────────

/// 创建 id 主键字段。
fn make_id_field() -> FieldDef {
    FieldDef {
        name: "id".into(),
        field_type: "text".into(),
        required: true,
        unique: false,
        computed: false,
        references: None,
        is_updatable: false,
        is_auto_generated: true,
        rust_type: "String".into(),
        sqlite_type: "TEXT PRIMARY KEY NOT NULL".into(),
    }
}

/// 创建 created_at 字段。
fn make_created_at_field() -> FieldDef {
    FieldDef {
        name: "created_at".into(),
        field_type: "timestamp".into(),
        required: true,
        unique: false,
        computed: false,
        references: None,
        is_updatable: false,
        is_auto_generated: true,
        rust_type: "String".into(),
        sqlite_type: "TEXT NOT NULL DEFAULT (datetime('now'))".into(),
    }
}

/// 创建 updated_at 字段。
fn make_updated_at_field() -> FieldDef {
    FieldDef {
        name: "updated_at".into(),
        field_type: "timestamp".into(),
        required: true,
        unique: false,
        computed: false,
        references: None,
        is_updatable: false,
        is_auto_generated: true,
        rust_type: "String".into(),
        sqlite_type: "TEXT NOT NULL DEFAULT (datetime('now'))".into(),
    }
}

/// 创建 deleted_at 字段。
fn make_deleted_at_field() -> FieldDef {
    FieldDef {
        name: "deleted_at".into(),
        field_type: "timestamp".into(),
        required: false,
        unique: false,
        computed: false,
        references: None,
        is_updatable: false,
        is_auto_generated: true,
        rust_type: "Option<String>".into(),
        sqlite_type: "TEXT".into(),
    }
}

/// 创建 sort_order 字段。
fn make_sort_order_field() -> FieldDef {
    FieldDef {
        name: "sort_order".into(),
        field_type: "integer".into(),
        required: true,
        unique: false,
        computed: false,
        references: None,
        is_updatable: true,
        is_auto_generated: false,
        rust_type: "i64".into(),
        sqlite_type: "INTEGER NOT NULL DEFAULT 0".into(),
    }
}

// ── 加工管线 ───────────────────────────────────────────────────────────────

/// 将 TOML 解析得到的原始字段加工：填充类型映射和 updatable 标记。
fn enrich_field(field: &mut FieldDef) {
    let (rust_ty, sqlite_ty) = map_type(&field.field_type, field.required);

    // relation 类型需要覆盖 map_type 的结果（始终 Option）
    if field.field_type == "relation" {
        field.rust_type = "Option<String>".into();
        field.sqlite_type = "TEXT".into();
    } else {
        field.rust_type = rust_ty.into();
        field.sqlite_type = sqlite_ty.into();
    }

    field.is_auto_generated = false;
    field.is_updatable = true;
}

/// 从 ColophonSchema 构建 TemplateContext。
///
/// # Errors
///
/// 如果 relation 类型字段声明了 `required = true`，返回错误。
/// relation 字段在 SQLite 中始终是可空的外键引用，required 语义无法保证。
pub fn build_context(schema: ColophonSchema) -> Result<TemplateContext> {
    // 预检：relation + required=true 是无效组合
    for field in &schema.fields {
        if field.field_type == "relation" && field.required {
            anyhow::bail!(
                "字段 '{}': type=\"relation\" 不支持 required=true。\
                 relation 字段在 SQLite 中始终是可空的外键引用，\
                 请将 required 改为 false。",
                field.name
            );
        }
    }

    let model_name = to_pascal_case(&schema.collection.name);
    let table_name = schema.collection.table.clone();
    let display_name = schema
        .collection
        .display_name
        .clone()
        .unwrap_or_else(|| model_name.clone());

    // 1. 注入 id 字段
    let mut all_fields: Vec<FieldDef> = Vec::with_capacity(schema.fields.len() + 5);
    all_fields.push(make_id_field());

    // 2. 加工用户定义的字段
    for mut field in schema.fields {
        enrich_field(&mut field);
        all_fields.push(field);
    }

    // 3. 展开 features
    expand_features(&schema.features, &mut all_fields);

    // 4. 构建视图字段列表
    let create_fields = build_template_fields_for_create(&all_fields);
    let update_fields = build_template_fields_for_update(&all_fields);
    let insert_fields = build_template_fields_for_insert(&all_fields);
    let select_columns = build_select_columns(&all_fields);

    // 5. 计算 INSERT/UPDATE 子句
    let insert_columns = insert_fields
        .iter()
        .map(|f| f.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let insert_placeholders = insert_fields
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(", ");
    let update_set_clause = update_fields
        .iter()
        .map(|f| format!("{} = ?", f.name))
        .collect::<Vec<_>>()
        .join(", ");

    Ok(TemplateContext {
        model_name,
        table_name,
        display_name,
        features: schema.features,
        fields: all_fields,
        create_fields,
        update_fields,
        insert_fields,
        select_columns,
        insert_columns,
        insert_placeholders,
        update_set_clause,
    })
}

/// 根据 features 开关注入系统字段。
fn expand_features(features: &FeaturesDef, fields: &mut Vec<FieldDef>) {
    if features.timestamps {
        fields.push(make_created_at_field());
        fields.push(make_updated_at_field());
    }
    if features.soft_delete {
        fields.push(make_deleted_at_field());
    }
    if features.sort_order {
        fields.push(make_sort_order_field());
    }
}

/// 将 FieldDef 转换为 TemplateField，并计算 create_type 和 update_type。
fn to_template_field(field: &FieldDef) -> TemplateField {
    let base_type = field.rust_type.clone();

    // create_type：required 字段用原类型，非 required 用 Option<原类型>
    let create_type = if field.required && !field.computed {
        base_type.clone()
    } else {
        // relation 类型已经是 Option<String>，不要再包一层
        if base_type.starts_with("Option<") {
            base_type.clone()
        } else {
            format!("Option<{}>", base_type)
        }
    };

    // update_type：始终 Option<原类型>
    let update_type = if base_type.starts_with("Option<") {
        base_type.clone()
    } else {
        format!("Option<{}>", base_type)
    };

    TemplateField {
        field: field.clone(),
        create_type,
        update_type,
    }
}

/// CreateDTO 字段：排除 computed 和 auto_generated 字段。
///
/// computed 字段由服务端计算，auto_generated 字段由数据库自动填充，
/// 均不应出现在创建请求中。
fn build_template_fields_for_create(fields: &[FieldDef]) -> Vec<TemplateField> {
    fields
        .iter()
        .filter(|f| !f.computed && !f.is_auto_generated)
        .map(to_template_field)
        .collect()
}

/// UpdateDTO 字段：仅包含 `is_updatable` 为 true 的字段。
///
/// 使用 `is_updatable` 标记而非硬编码名称或类型判断，
/// 由 FieldDef 在解析阶段统一决定哪些字段可更新。
fn build_template_fields_for_update(fields: &[FieldDef]) -> Vec<TemplateField> {
    fields
        .iter()
        .filter(|f| f.is_updatable)
        .map(to_template_field)
        .collect()
}

/// INSERT 语句字段：排除 auto_generated 字段。
///
/// auto_generated 字段（id、created_at 等）由数据库或服务端自动填充。
fn build_template_fields_for_insert(fields: &[FieldDef]) -> Vec<TemplateField> {
    fields
        .iter()
        .filter(|f| !f.is_auto_generated)
        .map(to_template_field)
        .collect()
}

/// 生成 SELECT 列列表，逗号分隔。
fn build_select_columns(fields: &[FieldDef]) -> String {
    fields
        .iter()
        .map(|f| f.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::schema::types::{CollectionDef, FeaturesDef, FieldDef};

    /// 辅助函数：构建一个最小的 ColophonSchema 用于测试。
    fn minimal_schema(fields: Vec<FieldDef>) -> ColophonSchema {
        ColophonSchema {
            collection: CollectionDef {
                name: "Category".into(),
                table: "categories".into(),
                display_name: Some("分类".into()),
            },
            features: FeaturesDef::default(),
            fields,
        }
    }

    /// 辅助函数：构建一个原始的用户字段（未加工状态）。
    fn raw_field(name: &str, field_type: &str) -> FieldDef {
        FieldDef {
            name: name.into(),
            field_type: field_type.into(),
            required: true,
            unique: false,
            computed: false,
            references: None,
            is_updatable: false,
            is_auto_generated: false,
            rust_type: String::new(),
            sqlite_type: String::new(),
        }
    }

    #[test]
    fn id_field_is_auto_injected() {
        let schema = minimal_schema(vec![raw_field("name", "text")]);
        let ctx = build_context(schema).expect("构建失败");

        let id = ctx.fields.first().expect("should have id field");
        assert_eq!(id.name, "id");
        assert!(id.is_auto_generated);
        assert!(!id.is_updatable);
        assert_eq!(id.rust_type, "String");
        assert_eq!(id.sqlite_type, "TEXT PRIMARY KEY NOT NULL");
    }

    #[test]
    fn text_type_mapping() {
        let schema = minimal_schema(vec![raw_field("slug", "text")]);
        let ctx = build_context(schema).expect("构建失败");

        let slug = ctx.fields.iter().find(|f| f.name == "slug").unwrap();
        assert_eq!(slug.rust_type, "String");
        assert_eq!(slug.sqlite_type, "TEXT NOT NULL");
    }

    #[test]
    fn richtext_type_mapping() {
        let schema = minimal_schema(vec![raw_field("content", "richtext")]);
        let ctx = build_context(schema).expect("构建失败");

        let f = ctx.fields.iter().find(|f| f.name == "content").unwrap();
        assert_eq!(f.rust_type, "String");
        assert_eq!(f.sqlite_type, "TEXT NOT NULL");
    }

    #[test]
    fn boolean_type_mapping() {
        let schema = minimal_schema(vec![raw_field("published", "boolean")]);
        let ctx = build_context(schema).expect("构建失败");

        let f = ctx.fields.iter().find(|f| f.name == "published").unwrap();
        assert_eq!(f.rust_type, "bool");
        assert_eq!(f.sqlite_type, "INTEGER NOT NULL DEFAULT 0");
    }

    #[test]
    fn integer_type_mapping() {
        let schema = minimal_schema(vec![raw_field("count", "integer")]);
        let ctx = build_context(schema).expect("构建失败");

        let f = ctx.fields.iter().find(|f| f.name == "count").unwrap();
        assert_eq!(f.rust_type, "i64");
        assert_eq!(f.sqlite_type, "INTEGER NOT NULL DEFAULT 0");
    }

    #[test]
    fn timestamp_type_mapping() {
        let schema = minimal_schema(vec![raw_field("published_at", "timestamp")]);
        let ctx = build_context(schema).expect("构建失败");

        let f = ctx.fields.iter().find(|f| f.name == "published_at").unwrap();
        assert_eq!(f.rust_type, "String");
        assert_eq!(
            f.sqlite_type,
            "TEXT NOT NULL DEFAULT (datetime('now'))"
        );
    }

    #[test]
    fn relation_type_rejects_required_true() {
        let mut field = raw_field("category_id", "relation");
        field.required = true;
        let schema = minimal_schema(vec![field]);
        let result = build_context(schema);

        assert!(result.is_err(), "relation + required=true 应被拒绝");
    }

    #[test]
    fn relation_type_optional_when_not_required() {
        let mut field = raw_field("parent_id", "relation");
        field.required = false;
        let schema = minimal_schema(vec![field]);
        let ctx = build_context(schema).expect("构建失败");

        let f = ctx.fields.iter().find(|f| f.name == "parent_id").unwrap();
        assert_eq!(f.rust_type, "Option<String>");
        assert_eq!(f.sqlite_type, "TEXT");
    }

    #[test]
    fn computed_field_excluded_from_create_fields() {
        let mut field = raw_field("excerpt", "text");
        field.computed = true;
        let schema = minimal_schema(vec![raw_field("title", "text"), field]);
        let ctx = build_context(schema).expect("构建失败");

        // create_fields 应排除 computed
        assert!(ctx.create_fields.iter().all(|f| f.name != "excerpt"));
        // 但 title 应存在
        assert!(ctx.create_fields.iter().any(|f| f.name == "title"));
    }

    #[test]
    fn id_field_excluded_from_update_fields() {
        let schema = minimal_schema(vec![raw_field("name", "text")]);
        let ctx = build_context(schema).expect("构建失败");

        assert!(ctx.update_fields.iter().all(|f| f.name != "id"));
        assert!(ctx.update_fields.iter().any(|f| f.name == "name"));
    }

    #[test]
    fn auto_generated_excluded_from_insert_fields() {
        let features = FeaturesDef {
            timestamps: true,
            ..Default::default()
        };
        let schema = ColophonSchema {
            collection: CollectionDef {
                name: "Post".into(),
                table: "posts".into(),
                display_name: None,
            },
            features,
            fields: vec![raw_field("title", "text")],
        };
        let ctx = build_context(schema).expect("构建失败");

        // insert_fields 不应包含 id、created_at、updated_at
        assert!(ctx.insert_fields.iter().all(|f| !f.is_auto_generated));
        // 但 title 应存在
        assert!(ctx.insert_fields.iter().any(|f| f.name == "title"));
    }

    #[test]
    fn timestamps_feature_injects_created_at_and_updated_at() {
        let features = FeaturesDef {
            timestamps: true,
            ..Default::default()
        };
        let schema = ColophonSchema {
            collection: CollectionDef {
                name: "Post".into(),
                table: "posts".into(),
                display_name: None,
            },
            features,
            fields: vec![raw_field("title", "text")],
        };
        let ctx = build_context(schema).expect("构建失败");

        assert!(ctx.fields.iter().any(|f| f.name == "created_at"));
        assert!(ctx.fields.iter().any(|f| f.name == "updated_at"));
    }

    #[test]
    fn soft_delete_feature_injects_deleted_at() {
        let features = FeaturesDef {
            soft_delete: true,
            ..Default::default()
        };
        let schema = ColophonSchema {
            collection: CollectionDef {
                name: "Post".into(),
                table: "posts".into(),
                display_name: None,
            },
            features,
            fields: vec![raw_field("title", "text")],
        };
        let ctx = build_context(schema).expect("构建失败");

        let deleted = ctx.fields.iter().find(|f| f.name == "deleted_at").unwrap();
        assert_eq!(deleted.rust_type, "Option<String>");
        assert_eq!(deleted.sqlite_type, "TEXT");
        assert!(!deleted.required);
    }

    #[test]
    fn sort_order_feature_injects_sort_order_field() {
        let features = FeaturesDef {
            sort_order: true,
            ..Default::default()
        };
        let schema = ColophonSchema {
            collection: CollectionDef {
                name: "Category".into(),
                table: "categories".into(),
                display_name: None,
            },
            features,
            fields: vec![raw_field("name", "text")],
        };
        let ctx = build_context(schema).expect("构建失败");

        let sort = ctx.fields.iter().find(|f| f.name == "sort_order").unwrap();
        assert_eq!(sort.rust_type, "i64");
        assert_eq!(sort.sqlite_type, "INTEGER NOT NULL DEFAULT 0");
        assert!(sort.is_updatable);
        assert!(!sort.is_auto_generated);
    }

    #[test]
    fn select_columns_contains_all_fields() {
        let schema = minimal_schema(vec![
            raw_field("name", "text"),
            raw_field("slug", "text"),
        ]);
        let ctx = build_context(schema).expect("构建失败");

        assert_eq!(ctx.select_columns, "id, name, slug");
    }

    #[test]
    fn display_name_falls_back_to_model_name() {
        let schema = ColophonSchema {
            collection: CollectionDef {
                name: "Tag".into(),
                table: "tags".into(),
                display_name: None,
            },
            features: FeaturesDef::default(),
            fields: vec![raw_field("name", "text")],
        };
        let ctx = build_context(schema).expect("构建失败");

        assert_eq!(ctx.display_name, "Tag");
    }

    #[test]
    fn user_fields_are_marked_updatable() {
        let schema = minimal_schema(vec![raw_field("name", "text")]);
        let ctx = build_context(schema).expect("构建失败");

        let name = ctx.fields.iter().find(|f| f.name == "name").unwrap();
        assert!(name.is_updatable);
        assert!(!name.is_auto_generated);
    }

    #[test]
    fn unknown_type_maps_to_string() {
        let schema = minimal_schema(vec![raw_field("meta", "json")]);
        let ctx = build_context(schema).expect("构建失败");

        let f = ctx.fields.iter().find(|f| f.name == "meta").unwrap();
        assert_eq!(f.rust_type, "String");
        assert_eq!(f.sqlite_type, "TEXT NOT NULL");
    }

    #[test]
    fn fix_l1_unknown_type_logs_warning() {
        // 测试未知类型仍然映射为 String（功能正确性）
        let schema = minimal_schema(vec![raw_field("meta", "texxt")]);
        let ctx = build_context(schema).expect("构建失败");

        let f = ctx.fields.iter().find(|f| f.name == "meta").unwrap();
        assert_eq!(f.rust_type, "String");
        assert_eq!(f.sqlite_type, "TEXT NOT NULL");
        // 注意：日志输出的验证需要 tracing-test crate 或自定义 subscriber
        // 此处验证功能正确性，日志验证通过代码审查确认
    }

    // ── to_pascal_case 测试 ────────────────────────────────────────────────────

    #[test]
    fn to_pascal_case_converts_snake_case() {
        assert_eq!(to_pascal_case("label"), "Label");
        assert_eq!(to_pascal_case("blog_post"), "BlogPost");
        assert_eq!(to_pascal_case("some_long_name"), "SomeLongName");
        assert_eq!(to_pascal_case("media_category"), "MediaCategory");
    }

    #[test]
    fn to_pascal_case_preserves_already_pascal_case() {
        assert_eq!(to_pascal_case("Category"), "Category");
        assert_eq!(to_pascal_case("Tag"), "Tag");
        // 已 PascalCase 的多词名称（无下划线）应原样返回
        assert_eq!(to_pascal_case("BlogPost"), "BlogPost");
    }

    #[test]
    fn to_pascal_case_handles_single_char() {
        assert_eq!(to_pascal_case("a"), "A");
        // "x_y" → "X" + "Y" = "XY"
        assert_eq!(to_pascal_case("x_y"), "XY");
    }

    #[test]
    fn fix_m2_to_pascal_case_boundary() {
        // 已 PascalCase 原样返回
        assert_eq!(to_pascal_case("Category"), "Category");
        // snake_case 正常转换
        assert_eq!(to_pascal_case("media_category"), "MediaCategory");
        // 单字符
        assert_eq!(to_pascal_case("a"), "A");
        // 已 PascalCase 多词（无下划线）原样返回
        assert_eq!(to_pascal_case("BlogPost"), "BlogPost");
    }

    #[test]
    fn fix_l6_pascal_case_empty_string() {
        // 空字符串应返回空字符串
        assert_eq!(to_pascal_case(""), "");
    }

    #[test]
    fn fix_l6_pascal_case_consecutive_underscores() {
        // 连续下划线应产生空字符串段，但不影响结果
        assert_eq!(to_pascal_case("a__b"), "AB");
        assert_eq!(to_pascal_case("some__name"), "SomeName");
        // 前导/尾随下划线
        assert_eq!(to_pascal_case("_test"), "Test");
        assert_eq!(to_pascal_case("test_"), "Test");
    }

    #[test]
    fn fix_l6_pascal_case_starts_with_digit() {
        // 数字开头的标识符
        assert_eq!(to_pascal_case("3d_model"), "3dModel");
        assert_eq!(to_pascal_case("2fa_code"), "2faCode");
        // 纯数字
        assert_eq!(to_pascal_case("123"), "123");
    }

    #[test]
    fn build_context_converts_lowercase_name_to_pascal_case() {
        let schema = ColophonSchema {
            collection: CollectionDef {
                name: "label".into(),
                table: "labels".into(),
                display_name: None,
            },
            features: FeaturesDef::default(),
            fields: vec![raw_field("name", "text")],
        };
        let ctx = build_context(schema).expect("构建失败");

        assert_eq!(ctx.model_name, "Label");
        // display_name 也应使用转换后的 PascalCase
        assert_eq!(ctx.display_name, "Label");
    }

    // ── 新增 TemplateField 类型测试 ───────────────────────────────────────────

    #[test]
    fn create_type_required_field_uses_base_type() {
        let schema = minimal_schema(vec![raw_field("name", "text")]);
        let ctx = build_context(schema).expect("构建失败");

        let name = ctx.create_fields.iter().find(|f| f.name == "name").unwrap();
        assert_eq!(name.create_type, "String");
    }

    #[test]
    fn create_type_optional_field_wraps_in_option() {
        let mut field = raw_field("description", "text");
        field.required = false;
        let schema = minimal_schema(vec![field]);
        let ctx = build_context(schema).expect("构建失败");

        let desc = ctx.create_fields.iter().find(|f| f.name == "description").unwrap();
        assert_eq!(desc.create_type, "Option<String>");
    }

    #[test]
    fn create_type_relation_stays_option() {
        let mut field = raw_field("parent_id", "relation");
        field.required = false;
        let schema = minimal_schema(vec![field]);
        let ctx = build_context(schema).expect("构建失败");

        let parent = ctx.create_fields.iter().find(|f| f.name == "parent_id").unwrap();
        assert_eq!(parent.create_type, "Option<String>");
    }

    #[test]
    fn update_type_always_option() {
        let schema = minimal_schema(vec![raw_field("name", "text")]);
        let ctx = build_context(schema).expect("构建失败");

        let name = ctx.update_fields.iter().find(|f| f.name == "name").unwrap();
        assert_eq!(name.update_type, "Option<String>");
    }

    #[test]
    fn update_type_relation_stays_option() {
        let mut field = raw_field("parent_id", "relation");
        field.required = false;
        let schema = minimal_schema(vec![field]);
        let ctx = build_context(schema).expect("构建失败");

        let parent = ctx.update_fields.iter().find(|f| f.name == "parent_id").unwrap();
        assert_eq!(parent.update_type, "Option<String>");
    }

    #[test]
    fn insert_columns_excludes_auto_generated() {
        let features = FeaturesDef {
            timestamps: true,
            ..Default::default()
        };
        let schema = ColophonSchema {
            collection: CollectionDef {
                name: "Post".into(),
                table: "posts".into(),
                display_name: None,
            },
            features,
            fields: vec![raw_field("title", "text")],
        };
        let ctx = build_context(schema).expect("构建失败");

        assert_eq!(ctx.insert_columns, "title");
        assert!(!ctx.insert_columns.contains("id"));
        assert!(!ctx.insert_columns.contains("created_at"));
    }

    #[test]
    fn insert_placeholders_matches_insert_fields_count() {
        let features = FeaturesDef {
            timestamps: true,
            ..Default::default()
        };
        let schema = ColophonSchema {
            collection: CollectionDef {
                name: "Post".into(),
                table: "posts".into(),
                display_name: None,
            },
            features,
            fields: vec![raw_field("title", "text"), raw_field("slug", "text")],
        };
        let ctx = build_context(schema).expect("构建失败");

        let placeholder_count = ctx.insert_placeholders.matches('?').count();
        assert_eq!(placeholder_count, ctx.insert_fields.len());
        assert_eq!(ctx.insert_placeholders, "?, ?");
    }

    #[test]
    fn update_set_clause_excludes_id() {
        let schema = minimal_schema(vec![raw_field("name", "text"), raw_field("slug", "text")]);
        let ctx = build_context(schema).expect("构建失败");

        assert!(!ctx.update_set_clause.contains("id = ?"));
        assert!(ctx.update_set_clause.contains("name = ?"));
        assert!(ctx.update_set_clause.contains("slug = ?"));
    }

    #[test]
    fn features_accessible_in_context() {
        let features = FeaturesDef {
            soft_delete: true,
            timestamps: true,
            sort_order: true,
        };
        let schema = ColophonSchema {
            collection: CollectionDef {
                name: "Category".into(),
                table: "categories".into(),
                display_name: None,
            },
            features,
            fields: vec![],
        };
        let ctx = build_context(schema).expect("构建失败");

        assert!(ctx.features.soft_delete);
        assert!(ctx.features.timestamps);
        assert!(ctx.features.sort_order);
    }

    // ── M1 修复测试 ──────────────────────────────────────────────────────────

    #[test]
    fn fix_m1_relation_rejects_required_true() {
        let mut field = raw_field("category_id", "relation");
        field.required = true;
        let schema = minimal_schema(vec![field]);

        let result = build_context(schema);
        assert!(result.is_err(), "relation + required=true 应返回 Err");

        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("relation") && err_msg.contains("required=true"),
            "错误信息应说明 relation 不支持 required=true: {}",
            err_msg
        );
    }

    #[test]
    fn fix_m1_relation_accepts_required_false() {
        let mut field = raw_field("parent_id", "relation");
        field.required = false;
        let schema = minimal_schema(vec![field]);

        let result = build_context(schema);
        assert!(result.is_ok(), "relation + required=false 应返回 Ok");

        let ctx = result.unwrap();
        let parent = ctx.fields.iter().find(|f| f.name == "parent_id").unwrap();
        assert_eq!(parent.rust_type, "Option<String>");
    }
}
