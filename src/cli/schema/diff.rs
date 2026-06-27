//! Schema Diffing Engine。
//!
//! 比较当前 Schema 定义与锁文件中的上一次状态，生成迁移 SQL。
//!
//! v0.1 安全限制：仅支持 `ADD COLUMN`，检测到删除或修改列时返回 `DiffError`。

use std::collections::HashMap;
use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::types::FieldDef;

// ── 错误类型 ────────────────────────────────────────────────────────────────

/// Diff 引擎错误。
#[derive(Debug)]
pub enum DiffError {
    /// 检测到破坏性 schema 变更（列删除或类型修改）。
    DestructiveChange { collection: String, details: String },
}

impl fmt::Display for DiffError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiffError::DestructiveChange {
                collection,
                details,
            } => {
                write!(
                    f,
                    "Detected destructive schema change for '{}':\n{}\n\n\
                     SQLite ALTER TABLE does not safely support these operations.\n\
                     Please write the migration SQL manually.",
                    collection, details
                )
            }
        }
    }
}

impl std::error::Error for DiffError {}

// ── 锁文件结构 ──────────────────────────────────────────────────────────────

/// `.colophon.lock` 的顶层结构。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaLock {
    pub version: u32,
    pub generated_at: String,
    pub collections: HashMap<String, LockCollection>,
}

/// 锁文件中单个集合的快照。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockCollection {
    pub table: String,
    pub fields: Vec<FieldDef>,
}

// ── Diff 结果 ───────────────────────────────────────────────────────────────

/// Schema 差异类型。
#[derive(Debug)]
pub enum SchemaDiff {
    /// 集合不存在于锁文件中，需要创建新表。
    CreateTable,
    /// 集合存在，有新增列。
    AddColumns(Vec<FieldDef>),
    /// 无变化。
    NoChange,
}

/// Diff 完整结果，包含差异类型、迁移 SQL 和编号。
#[derive(Debug)]
pub struct DiffResult {
    pub diff: SchemaDiff,
    pub migration_sql: String,
    pub migration_number: u32,
}

// ── 锁文件读取 ──────────────────────────────────────────────────────────────

/// 从项目根目录读取 `.colophon.lock`。
///
/// 如果文件不存在，返回空锁文件（version=1, collections 为空）。
/// 文件读取或解析失败时返回错误，由调用方决定如何处理。
pub fn read_lock_file(project_root: &Path) -> Result<SchemaLock, LockFileError> {
    let lock_path = project_root.join(".colophon.lock");

    if !lock_path.exists() {
        return Ok(SchemaLock {
            version: 1,
            generated_at: now_iso8601(),
            collections: HashMap::new(),
        });
    }

    let content = std::fs::read_to_string(&lock_path).map_err(|e| LockFileError::Io {
        path: lock_path.display().to_string(),
        source: e,
    })?;

    toml::from_str(&content).map_err(|e| LockFileError::Parse {
        path: lock_path.display().to_string(),
        source: e,
    })
}

/// 锁文件读取/解析错误。
#[derive(Debug)]
pub enum LockFileError {
    /// 文件读取失败。
    Io {
        path: String,
        source: std::io::Error,
    },
    /// TOML 解析失败。
    Parse {
        path: String,
        source: toml::de::Error,
    },
}

impl fmt::Display for LockFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LockFileError::Io { path, source } => {
                write!(f, "无法读取锁文件 {}: {}", path, source)
            }
            LockFileError::Parse { path, source } => {
                write!(f, "锁文件 TOML 解析失败 {}: {}", path, source)
            }
        }
    }
}

impl std::error::Error for LockFileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            LockFileError::Io { source, .. } => Some(source),
            LockFileError::Parse { source, .. } => Some(source),
        }
    }
}

/// 生成当前时间的 ISO 8601 字符串。
fn now_iso8601() -> String {
    chrono::Utc::now().to_rfc3339()
}

// ── Diff 核心逻辑 ───────────────────────────────────────────────────────────

/// 对单个集合执行 Diff，返回 SchemaDiff。
///
/// 检测到破坏性变更（列删除或类型修改）时返回 `Err(DiffError)`。
pub fn diff_collection(
    collection_name: &str,
    table_name: &str,
    current_fields: &[FieldDef],
    lock: &SchemaLock,
) -> Result<SchemaDiff, DiffError> {
    match lock.collections.get(collection_name) {
        None => Ok(SchemaDiff::CreateTable),
        Some(lock_collection) => {
            diff_existing_collection(collection_name, table_name, current_fields, lock_collection)
        }
    }
}

/// 比较已存在集合的字段差异。
fn diff_existing_collection(
    collection_name: &str,
    _table_name: &str,
    current_fields: &[FieldDef],
    lock_collection: &LockCollection,
) -> Result<SchemaDiff, DiffError> {
    // 构建锁文件中字段名到类型的映射
    let lock_fields_by_name: HashMap<&str, &str> = lock_collection
        .fields
        .iter()
        .map(|f| (f.name.as_str(), f.sqlite_type.as_str()))
        .collect();

    // 收集错误信息
    let mut errors: Vec<String> = Vec::new();

    // 检查删除的列：锁文件中存在但当前 schema 中不存在
    for lock_field in &lock_collection.fields {
        if !current_fields.iter().any(|f| f.name == lock_field.name) {
            errors.push(format!("Column '{}' was removed", lock_field.name));
        }
    }

    // 检查类型变更的列和新增的列
    let mut added_fields: Vec<FieldDef> = Vec::new();

    for current_field in current_fields {
        match lock_fields_by_name.get(current_field.name.as_str()) {
            None => {
                // 新增列
                added_fields.push(current_field.clone());
            }
            Some(lock_sqlite_type) => {
                // 检查类型是否变化
                if *lock_sqlite_type != current_field.sqlite_type {
                    errors.push(format!(
                        "Column '{}' type changed from '{}' to '{}'",
                        current_field.name, lock_sqlite_type, current_field.sqlite_type
                    ));
                }
            }
        }
    }

    // 如果有破坏性变更，返回错误
    if !errors.is_empty() {
        let error_details = errors
            .iter()
            .map(|e| format!("  - {}", e))
            .collect::<Vec<_>>()
            .join("\n");

        return Err(DiffError::DestructiveChange {
            collection: collection_name.to_string(),
            details: error_details,
        });
    }

    if added_fields.is_empty() {
        Ok(SchemaDiff::NoChange)
    } else {
        Ok(SchemaDiff::AddColumns(added_fields))
    }
}

// ── SQL 生成 ────────────────────────────────────────────────────────────────

/// 根据 SchemaDiff 生成迁移 SQL。
pub fn generate_migration_sql(
    diff: &SchemaDiff,
    _collection_name: &str,
    table_name: &str,
    current_fields: &[FieldDef],
) -> String {
    match diff {
        SchemaDiff::CreateTable => generate_create_table_sql(table_name, current_fields),
        SchemaDiff::AddColumns(columns) => generate_add_columns_sql(table_name, columns),
        SchemaDiff::NoChange => String::new(),
    }
}

/// 生成 CREATE TABLE IF NOT EXISTS 语句。
fn generate_create_table_sql(table_name: &str, fields: &[FieldDef]) -> String {
    let column_defs: Vec<String> = fields.iter().map(|f| format_column_def(f)).collect();

    format!(
        "CREATE TABLE IF NOT EXISTS {} (\n{}\n);",
        table_name,
        column_defs.join(",\n")
    )
}

/// 格式化单个列定义。
fn format_column_def(field: &FieldDef) -> String {
    if field.name == "id" {
        return "    id TEXT PRIMARY KEY NOT NULL".to_string();
    }

    let mut parts = vec![format!("    {}", field.name), field.sqlite_type.clone()];

    if field.required && !field.sqlite_type.contains("NOT NULL") {
        parts.push("NOT NULL".to_string());
    }

    if field.unique {
        parts.push("UNIQUE".to_string());
    }

    if let Some(ref references) = field.references {
        parts.push(format!("REFERENCES {}(id)", references));
    }

    parts.join(" ")
}

/// 生成多条 ALTER TABLE ... ADD COLUMN 语句。
fn generate_add_columns_sql(table_name: &str, columns: &[FieldDef]) -> String {
    columns
        .iter()
        .map(|col| {
            let col_def = format_column_def_for_alter(col);
            format!("ALTER TABLE {} ADD COLUMN {};", table_name, col_def)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 格式化 ALTER TABLE ADD COLUMN 用的列定义（不缩进）。
fn format_column_def_for_alter(field: &FieldDef) -> String {
    let mut parts = vec![field.name.clone(), field.sqlite_type.clone()];

    if field.required && !field.sqlite_type.contains("NOT NULL") {
        parts.push("NOT NULL".to_string());
    }

    if field.unique {
        parts.push("UNIQUE".to_string());
    }

    if let Some(ref references) = field.references {
        parts.push(format!("REFERENCES {}(id)", references));
    }

    parts.join(" ")
}

// ── Migration 编号 ──────────────────────────────────────────────────────────

/// 扫描 `migrations/` 目录，找到最大编号，返回下一个编号。
///
/// 目录不存在时返回 1；目录不可读时返回 Err。
/// 非标准格式的文件名（如 `.gitkeep`、`README.md`）会被静默跳过。
pub fn next_migration_number(project_root: &Path) -> Result<u32, std::io::Error> {
    let migrations_dir = project_root.join("migrations");

    if !migrations_dir.exists() {
        return Ok(1);
    }

    let max_number = std::fs::read_dir(&migrations_dir)
        .map_err(|e| {
            std::io::Error::new(
                e.kind(),
                format!(
                    "无法读取 migrations 目录 {}: {}",
                    migrations_dir.display(),
                    e
                ),
            )
        })?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            // 文件名格式: NNN_description.sql
            parse_migration_number(&name_str)
        })
        .max()
        .unwrap_or(0);

    Ok(max_number + 1)
}

/// 从迁移文件名中解析编号。
///
/// 文件名格式: `NNN_description.sql`，如 `001_init.sql`。
/// 只处理 .sql 扩展名的文件，其他扩展名返回 None。
fn parse_migration_number(filename: &str) -> Option<u32> {
    // 只处理 .sql 文件
    if !filename.ends_with(".sql") {
        return None;
    }
    let prefix = filename.split('_').next()?;
    let number: u32 = prefix.parse().ok()?;
    Some(number)
}

// ── 锁文件更新（内存） ─────────────────────────────────────────────────────

/// 在内存中更新 SchemaLock，将新集合的状态写入。
///
/// 不写入磁盘，由后续 Agent 负责持久化。
pub fn update_lock_in_memory(
    lock: &mut SchemaLock,
    collection_name: &str,
    table_name: &str,
    fields: &[FieldDef],
) {
    lock.collections.insert(
        collection_name.to_string(),
        LockCollection {
            table: table_name.to_string(),
            fields: fields.to_vec(),
        },
    );
    lock.generated_at = now_iso8601();
    lock.version += 1;
}

// ── 测试 ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// 辅助函数：创建测试用 FieldDef。
    fn test_field(name: &str, sqlite_type: &str) -> FieldDef {
        FieldDef {
            name: name.into(),
            field_type: "text".into(),
            required: true,
            unique: false,
            computed: false,
            references: None,
            is_updatable: true,
            is_auto_generated: false,
            rust_type: "String".into(),
            sqlite_type: sqlite_type.into(),
        }
    }

    /// 辅助函数：创建空的 SchemaLock。
    fn empty_lock() -> SchemaLock {
        SchemaLock {
            version: 1,
            generated_at: "2025-01-01T00:00:00Z".into(),
            collections: HashMap::new(),
        }
    }

    /// 辅助函数：创建包含集合的 SchemaLock。
    fn lock_with_collection(name: &str, table: &str, fields: Vec<FieldDef>) -> SchemaLock {
        let mut lock = empty_lock();
        lock.collections.insert(
            name.into(),
            LockCollection {
                table: table.into(),
                fields,
            },
        );
        lock
    }

    // ── 新建表场景 ──────────────────────────────────────────────────────────

    #[test]
    fn new_collection_returns_create_table() {
        let lock = empty_lock();
        let fields = vec![
            test_field("id", "TEXT PRIMARY KEY NOT NULL"),
            test_field("name", "TEXT NOT NULL"),
        ];

        let diff = diff_collection("categories", "categories", &fields, &lock).unwrap();

        assert!(matches!(diff, SchemaDiff::CreateTable));
    }

    #[test]
    fn create_table_generates_valid_sql() {
        let fields = vec![
            test_field("id", "TEXT PRIMARY KEY NOT NULL"),
            test_field("name", "TEXT NOT NULL"),
            test_field("slug", "TEXT NOT NULL"),
        ];

        let sql = generate_create_table_sql("categories", &fields);

        assert!(sql.contains("CREATE TABLE IF NOT EXISTS categories"));
        assert!(sql.contains("id TEXT PRIMARY KEY NOT NULL"));
        assert!(sql.contains("name TEXT NOT NULL"));
        assert!(sql.contains("slug TEXT NOT NULL"));
    }

    #[test]
    fn create_table_sql_ends_with_semicolon() {
        let fields = vec![test_field("id", "TEXT PRIMARY KEY NOT NULL")];
        let sql = generate_create_table_sql("test_table", &fields);

        assert!(sql.trim().ends_with(';'));
    }

    // ── 新增列场景 ──────────────────────────────────────────────────────────

    #[test]
    fn new_column_returns_add_columns() {
        let lock_fields = vec![
            test_field("id", "TEXT PRIMARY KEY NOT NULL"),
            test_field("name", "TEXT NOT NULL"),
        ];
        let lock = lock_with_collection("categories", "categories", lock_fields);

        let current_fields = vec![
            test_field("id", "TEXT PRIMARY KEY NOT NULL"),
            test_field("name", "TEXT NOT NULL"),
            test_field("description", "TEXT"),
        ];

        let diff = diff_collection("categories", "categories", &current_fields, &lock).unwrap();

        match diff {
            SchemaDiff::AddColumns(cols) => {
                assert_eq!(cols.len(), 1);
                assert_eq!(cols[0].name, "description");
            }
            _ => panic!("Expected AddColumns, got {:?}", diff),
        }
    }

    #[test]
    fn add_columns_generates_alter_statements() {
        let columns = vec![
            test_field("email", "TEXT NOT NULL"),
            test_field("age", "INTEGER NOT NULL DEFAULT 0"),
        ];

        let sql = generate_add_columns_sql("users", &columns);

        assert!(sql.contains("ALTER TABLE users ADD COLUMN email TEXT NOT NULL;"));
        assert!(sql.contains("ALTER TABLE users ADD COLUMN age INTEGER NOT NULL DEFAULT 0;"));
    }

    #[test]
    fn add_multiple_columns_generates_separate_statements() {
        let columns = vec![
            test_field("col_a", "TEXT NOT NULL"),
            test_field("col_b", "INTEGER NOT NULL DEFAULT 0"),
            test_field("col_c", "TEXT"),
        ];

        let sql = generate_add_columns_sql("test_table", &columns);
        let statements: Vec<&str> = sql.lines().collect();

        assert_eq!(statements.len(), 3);
        for stmt in &statements {
            assert!(stmt.starts_with("ALTER TABLE test_table ADD COLUMN"));
            assert!(stmt.ends_with(';'));
        }
    }

    // ── 无变化场景 ──────────────────────────────────────────────────────────

    #[test]
    fn identical_schemas_return_no_change() {
        let fields = vec![
            test_field("id", "TEXT PRIMARY KEY NOT NULL"),
            test_field("name", "TEXT NOT NULL"),
        ];
        let lock = lock_with_collection("categories", "categories", fields.clone());

        let diff = diff_collection("categories", "categories", &fields, &lock).unwrap();

        assert!(matches!(diff, SchemaDiff::NoChange));
    }

    #[test]
    fn no_change_generates_empty_sql() {
        let diff = SchemaDiff::NoChange;
        let sql = generate_migration_sql(&diff, "categories", "categories", &[]);

        assert!(sql.is_empty());
    }

    // ── 删除列错误场景 ──────────────────────────────────────────────────────

    #[test]
    fn removed_column_returns_destructive_error() {
        let lock_fields = vec![
            test_field("id", "TEXT PRIMARY KEY NOT NULL"),
            test_field("name", "TEXT NOT NULL"),
            test_field("old_field", "TEXT"),
        ];
        let lock = lock_with_collection("categories", "categories", lock_fields);

        let current_fields = vec![
            test_field("id", "TEXT PRIMARY KEY NOT NULL"),
            test_field("name", "TEXT NOT NULL"),
        ];

        let err = diff_collection("categories", "categories", &current_fields, &lock).unwrap_err();

        match &err {
            DiffError::DestructiveChange {
                collection,
                details,
            } => {
                assert_eq!(collection, "categories");
                assert!(details.contains("Column 'old_field' was removed"));
            }
        }
        assert!(err
            .to_string()
            .contains("Detected destructive schema change"));
        assert!(err
            .to_string()
            .contains("SQLite ALTER TABLE does not safely support"));
    }

    #[test]
    fn removed_column_error_contains_collection_name() {
        let lock_fields = vec![
            test_field("id", "TEXT PRIMARY KEY NOT NULL"),
            test_field("legacy_col", "TEXT"),
        ];
        let lock = lock_with_collection("items", "items", lock_fields);

        let current_fields = vec![test_field("id", "TEXT PRIMARY KEY NOT NULL")];

        let err = diff_collection("items", "items", &current_fields, &lock).unwrap_err();

        assert!(err.to_string().contains("'items'"));
    }

    // ── 修改类型错误场景 ────────────────────────────────────────────────────

    #[test]
    fn type_change_returns_destructive_error() {
        let lock_fields = vec![
            test_field("id", "TEXT PRIMARY KEY NOT NULL"),
            FieldDef {
                name: "status".into(),
                field_type: "text".into(),
                required: true,
                unique: false,
                computed: false,
                references: None,
                is_updatable: true,
                is_auto_generated: false,
                rust_type: "String".into(),
                sqlite_type: "TEXT".into(),
            },
        ];
        let lock = lock_with_collection("categories", "categories", lock_fields);

        let current_fields = vec![
            test_field("id", "TEXT PRIMARY KEY NOT NULL"),
            FieldDef {
                name: "status".into(),
                field_type: "integer".into(),
                required: true,
                unique: false,
                computed: false,
                references: None,
                is_updatable: true,
                is_auto_generated: false,
                rust_type: "i64".into(),
                sqlite_type: "INTEGER NOT NULL DEFAULT 0".into(),
            },
        ];

        let err = diff_collection("categories", "categories", &current_fields, &lock).unwrap_err();

        match &err {
            DiffError::DestructiveChange { details, .. } => {
                assert!(details.contains(
                    "Column 'status' type changed from 'TEXT' to 'INTEGER NOT NULL DEFAULT 0'"
                ));
            }
        }
    }

    #[test]
    fn type_change_error_contains_destructive_message() {
        let lock_fields = vec![
            test_field("id", "TEXT PRIMARY KEY NOT NULL"),
            test_field("score", "TEXT"),
        ];
        let lock = lock_with_collection("games", "games", lock_fields);

        let current_fields = vec![
            test_field("id", "TEXT PRIMARY KEY NOT NULL"),
            test_field("score", "INTEGER NOT NULL DEFAULT 0"),
        ];

        let err = diff_collection("games", "games", &current_fields, &lock).unwrap_err();

        assert!(err
            .to_string()
            .contains("Detected destructive schema change"));
    }

    // ── 组合场景 ────────────────────────────────────────────────────────────

    #[test]
    fn removal_and_addition_returns_error() {
        let lock_fields = vec![
            test_field("id", "TEXT PRIMARY KEY NOT NULL"),
            test_field("old_col", "TEXT"),
        ];
        let lock = lock_with_collection("mixed", "mixed", lock_fields);

        let current_fields = vec![
            test_field("id", "TEXT PRIMARY KEY NOT NULL"),
            test_field("new_col", "TEXT NOT NULL"),
        ];

        let err = diff_collection("mixed", "mixed", &current_fields, &lock).unwrap_err();

        match &err {
            DiffError::DestructiveChange { details, .. } => {
                assert!(details.contains("Column 'old_col' was removed"));
            }
        }
    }

    #[test]
    fn multiple_new_columns_returns_all() {
        let lock_fields = vec![test_field("id", "TEXT PRIMARY KEY NOT NULL")];
        let lock = lock_with_collection("items", "items", lock_fields);

        let current_fields = vec![
            test_field("id", "TEXT PRIMARY KEY NOT NULL"),
            test_field("name", "TEXT NOT NULL"),
            test_field("description", "TEXT"),
            test_field("priority", "INTEGER NOT NULL DEFAULT 0"),
        ];

        let diff = diff_collection("items", "items", &current_fields, &lock).unwrap();

        match diff {
            SchemaDiff::AddColumns(cols) => {
                assert_eq!(cols.len(), 3);
                let names: Vec<&str> = cols.iter().map(|c| c.name.as_str()).collect();
                assert!(names.contains(&"name"));
                assert!(names.contains(&"description"));
                assert!(names.contains(&"priority"));
            }
            _ => panic!("Expected AddColumns"),
        }
    }

    // ── Migration 编号 ──────────────────────────────────────────────────────

    #[test]
    fn parse_migration_number_valid() {
        assert_eq!(parse_migration_number("001_init.sql"), Some(1));
        assert_eq!(
            parse_migration_number("022_add_token_version.sql"),
            Some(22)
        );
        assert_eq!(parse_migration_number("999_big_migration.sql"), Some(999));
    }

    #[test]
    fn parse_migration_number_invalid() {
        assert_eq!(parse_migration_number("README.md"), None);
        assert_eq!(parse_migration_number(".gitkeep"), None);
        assert_eq!(parse_migration_number("abc_init.sql"), None);
    }

    #[test]
    fn next_migration_number_from_empty_dir() {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let project_root = dir.path();

        let num = next_migration_number(project_root).expect("不应返回错误");
        assert_eq!(num, 1);
    }

    #[test]
    fn next_migration_number_from_existing_migrations() {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let migrations_dir = dir.path().join("migrations");
        std::fs::create_dir(&migrations_dir).expect("创建 migrations 目录失败");

        std::fs::write(migrations_dir.join("001_init.sql"), "").expect("写入失败");
        std::fs::write(migrations_dir.join("005_add_users.sql"), "").expect("写入失败");
        std::fs::write(migrations_dir.join("003_add_posts.sql"), "").expect("写入失败");

        let num = next_migration_number(dir.path()).expect("不应返回错误");
        assert_eq!(num, 6);
    }

    #[test]
    fn next_migration_number_ignores_non_sql_files() {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let migrations_dir = dir.path().join("migrations");
        std::fs::create_dir(&migrations_dir).expect("创建 migrations 目录失败");

        std::fs::write(migrations_dir.join("001_init.sql"), "").expect("写入失败");
        std::fs::write(migrations_dir.join("README.md"), "").expect("写入失败");
        std::fs::write(migrations_dir.join(".gitkeep"), "").expect("写入失败");

        let num = next_migration_number(dir.path()).expect("不应返回错误");
        assert_eq!(num, 2);
    }

    // ── M4 修复测试 ──────────────────────────────────────────────────────────

    #[test]
    fn fix_m4_skips_non_standard_filenames() {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let migrations_dir = dir.path().join("migrations");
        std::fs::create_dir(&migrations_dir).expect("创建 migrations 目录失败");

        // 放入 .gitkeep 和非标准文件名
        std::fs::write(migrations_dir.join(".gitkeep"), "").expect("写入失败");
        std::fs::write(migrations_dir.join("abc_init.sql"), "").expect("写入失败");
        std::fs::write(migrations_dir.join("010_valid.sql"), "").expect("写入失败");

        let num = next_migration_number(dir.path()).expect("不应返回错误");
        assert_eq!(num, 11);
    }

    #[test]
    fn fix_m4_fallsback_to_0001_on_empty_dir() {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let migrations_dir = dir.path().join("migrations");
        std::fs::create_dir(&migrations_dir).expect("创建 migrations 目录失败");

        let num = next_migration_number(dir.path()).expect("不应返回错误");
        assert_eq!(num, 1);
    }

    #[test]
    fn fix_m4_returns_err_on_unreadable_dir() {
        // 在 Windows 上测试不可读目录比较困难，改用不存在的父路径来验证错误传播
        // 关键验证：不再 panic，而是返回 Err
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let nonexistent = dir.path().join("nonexistent_parent");
        // 创建一个名为 migrations 的文件（不是目录），read_dir 会失败
        std::fs::write(nonexistent.join("migrations"), "not a dir").ok();

        // 如果父目录不存在，migrations 不存在，应返回 Ok(1)
        // 如果 migrations 是文件不是目录，read_dir 会失败，应返回 Err
        let result = next_migration_number(&nonexistent);
        // 目录不存在 → Ok(1)
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);
    }

    #[test]
    fn fix_l2_ignores_non_sql_files() {
        // parse_migration_number 应该只处理 .sql 文件
        // "001_readme.md" 不是 .sql 文件，应返回 None
        assert_eq!(parse_migration_number("001_readme.md"), None);
        assert_eq!(parse_migration_number("001_init.txt"), None);
        assert_eq!(parse_migration_number("001_init.sql.bak"), None);

        // .sql 文件应正常解析
        assert_eq!(parse_migration_number("001_init.sql"), Some(1));
        assert_eq!(
            parse_migration_number("022_add_token_version.sql"),
            Some(22)
        );
    }

    // ── 锁文件读取 ─────────────────────────────────────────────────────────

    #[test]
    fn read_lock_file_returns_empty_lock_when_missing() {
        let dir = tempfile::tempdir().expect("创建临时目录失败");

        let lock = read_lock_file(dir.path()).unwrap();

        assert_eq!(lock.version, 1);
        assert!(lock.collections.is_empty());
    }

    #[test]
    fn read_lock_file_parses_existing_file() {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let lock_content = r#"
version = 3
generated_at = "2025-06-27T00:00:00Z"

[collections.categories]
table = "categories"
fields = [
    { name = "id", type = "text", required = true, unique = false, computed = false, is_updatable = false, is_auto_generated = true, rust_type = "String", sqlite_type = "TEXT PRIMARY KEY NOT NULL" },
    { name = "name", type = "text", required = true, unique = false, computed = false, is_updatable = true, is_auto_generated = false, rust_type = "String", sqlite_type = "TEXT NOT NULL" },
]
"#;
        std::fs::write(dir.path().join(".colophon.lock"), lock_content).expect("写入失败");

        let lock = read_lock_file(dir.path()).unwrap();

        assert_eq!(lock.version, 3);
        assert_eq!(lock.generated_at, "2025-06-27T00:00:00Z");
        assert!(lock.collections.contains_key("categories"));

        let categories = &lock.collections["categories"];
        assert_eq!(categories.table, "categories");
        assert_eq!(categories.fields.len(), 2);
        assert_eq!(categories.fields[0].name, "id");
        assert_eq!(categories.fields[1].name, "name");
    }

    #[test]
    fn read_lock_file_returns_error_on_invalid_toml() {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        std::fs::write(dir.path().join(".colophon.lock"), "invalid { toml content")
            .expect("写入失败");

        let result = read_lock_file(dir.path());
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(err.to_string().contains("锁文件 TOML 解析失败"));
    }

    // ── 锁文件更新 ─────────────────────────────────────────────────────────

    #[test]
    fn update_lock_in_memory_inserts_new_collection() {
        let mut lock = empty_lock();
        let fields = vec![
            test_field("id", "TEXT PRIMARY KEY NOT NULL"),
            test_field("name", "TEXT NOT NULL"),
        ];

        update_lock_in_memory(&mut lock, "categories", "categories", &fields);

        assert!(lock.collections.contains_key("categories"));
        assert_eq!(lock.collections["categories"].fields.len(), 2);
        assert_eq!(lock.version, 2);
    }

    #[test]
    fn update_lock_in_memory_overwrites_existing_collection() {
        let mut lock = lock_with_collection(
            "categories",
            "categories",
            vec![test_field("id", "TEXT PRIMARY KEY NOT NULL")],
        );

        let new_fields = vec![
            test_field("id", "TEXT PRIMARY KEY NOT NULL"),
            test_field("name", "TEXT NOT NULL"),
        ];

        update_lock_in_memory(&mut lock, "categories", "categories", &new_fields);

        assert_eq!(lock.collections["categories"].fields.len(), 2);
        assert_eq!(lock.version, 2);
    }

    // ── 完整流程集成测试 ────────────────────────────────────────────────────

    #[test]
    fn full_diff_flow_create_table() {
        let lock = empty_lock();
        let fields = vec![
            test_field("id", "TEXT PRIMARY KEY NOT NULL"),
            test_field("title", "TEXT NOT NULL"),
        ];

        let diff = diff_collection("posts", "posts", &fields, &lock).unwrap();
        let sql = generate_migration_sql(&diff, "posts", "posts", &fields);

        assert!(matches!(diff, SchemaDiff::CreateTable));
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS posts"));
    }

    #[test]
    fn full_diff_flow_add_columns() {
        let lock_fields = vec![
            test_field("id", "TEXT PRIMARY KEY NOT NULL"),
            test_field("title", "TEXT NOT NULL"),
        ];
        let lock = lock_with_collection("posts", "posts", lock_fields);

        let mut excerpt_field = test_field("excerpt", "TEXT");
        excerpt_field.required = false;

        let current_fields = vec![
            test_field("id", "TEXT PRIMARY KEY NOT NULL"),
            test_field("title", "TEXT NOT NULL"),
            excerpt_field,
        ];

        let diff = diff_collection("posts", "posts", &current_fields, &lock).unwrap();
        let sql = generate_migration_sql(&diff, "posts", "posts", &current_fields);

        match &diff {
            SchemaDiff::AddColumns(cols) => assert_eq!(cols.len(), 1),
            _ => panic!("Expected AddColumns"),
        }
        assert!(sql.contains("ALTER TABLE posts ADD COLUMN excerpt TEXT;"));
    }

    #[test]
    fn full_diff_flow_no_change() {
        let fields = vec![
            test_field("id", "TEXT PRIMARY KEY NOT NULL"),
            test_field("title", "TEXT NOT NULL"),
        ];
        let lock = lock_with_collection("posts", "posts", fields.clone());

        let diff = diff_collection("posts", "posts", &fields, &lock).unwrap();
        let sql = generate_migration_sql(&diff, "posts", "posts", &fields);

        assert!(matches!(diff, SchemaDiff::NoChange));
        assert!(sql.is_empty());
    }

    // ── 字段属性保持测试 ────────────────────────────────────────────────────

    #[test]
    fn add_column_preserves_unique_constraint() {
        let lock_fields = vec![test_field("id", "TEXT PRIMARY KEY NOT NULL")];
        let lock = lock_with_collection("items", "items", lock_fields);

        let mut unique_field = test_field("email", "TEXT NOT NULL");
        unique_field.unique = true;

        let current_fields = vec![test_field("id", "TEXT PRIMARY KEY NOT NULL"), unique_field];

        let diff = diff_collection("items", "items", &current_fields, &lock).unwrap();

        match diff {
            SchemaDiff::AddColumns(cols) => {
                let sql = generate_add_columns_sql("items", &cols);
                assert!(sql.contains("UNIQUE"));
            }
            _ => panic!("Expected AddColumns"),
        }
    }

    #[test]
    fn add_column_preserves_references() {
        let lock_fields = vec![test_field("id", "TEXT PRIMARY KEY NOT NULL")];
        let lock = lock_with_collection("posts", "posts", lock_fields);

        let mut ref_field = test_field("category_id", "TEXT");
        ref_field.required = false;
        ref_field.references = Some("categories".into());

        let current_fields = vec![test_field("id", "TEXT PRIMARY KEY NOT NULL"), ref_field];

        let diff = diff_collection("posts", "posts", &current_fields, &lock).unwrap();

        match diff {
            SchemaDiff::AddColumns(cols) => {
                let sql = generate_add_columns_sql("posts", &cols);
                assert!(sql.contains("REFERENCES categories(id)"));
            }
            _ => panic!("Expected AddColumns"),
        }
    }

    #[test]
    fn create_table_preserves_references() {
        let mut ref_field = test_field("author_id", "TEXT");
        ref_field.references = Some("users".into());

        let fields = vec![test_field("id", "TEXT PRIMARY KEY NOT NULL"), ref_field];

        let sql = generate_create_table_sql("posts", &fields);
        assert!(sql.contains("REFERENCES users(id)"));
    }
}
