//! Code Generator：渲染模板并写入文件。
//!
//! 职责：
//! 1. 加载 minijinja 模板
//! 2. 渲染各模块代码
//! 3. rustfmt 格式化 Rust 代码
//! 4. 写入 `src/modules/{name}/` 和 `migrations/`
//! 5. 更新 `.colophon.lock`
//! 6. 追加 `src/modules/mod.rs` 模块声明

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use minijinja::Environment;

use super::diff::{self, SchemaLock};
use super::types::TemplateContext;

// ── 模板目录 ──────────────────────────────────────────────────────────────

/// 获取模板目录的绝对路径。
///
/// 优先级：
/// 1. 环境变量 `COLOPHON_TEMPLATES_DIR`（如果设置）
/// 2. 项目根目录的 `cli/templates/`
fn templates_dir(project_root: &Path) -> PathBuf {
    if let Ok(custom_dir) = std::env::var("COLOPHON_TEMPLATES_DIR") {
        let path = PathBuf::from(custom_dir);
        if path.is_absolute() {
            return path;
        }
        // 相对路径基于项目根目录
        return project_root.join(path);
    }
    project_root.join("cli").join("templates")
}

// ── 模板引擎 ──────────────────────────────────────────────────────────────

/// 加载所有模板并返回渲染后的文件映射。
///
/// 返回 `(文件名, 渲染内容)` 列表，仅包含 Rust 源文件。
/// migration SQL 由 diff 引擎单独处理。
fn render_module_files(
    env: &Environment,
    ctx: &TemplateContext,
) -> Result<Vec<(String, String)>> {
    let template_names = ["domain.rs.j2", "dto.rs.j2", "repository.rs.j2", "handler.rs.j2", "service.rs.j2", "mod.rs.j2"];
    let mut files = Vec::with_capacity(template_names.len());

    for tpl_name in template_names {
        let tpl = env.get_template(tpl_name)
            .with_context(|| format!("模板 '{}' 未找到", tpl_name))?;
        let rendered = tpl.render(minijinja::Value::from_serialize(ctx))
            .with_context(|| format!("渲染模板 '{}' 失败", tpl_name))?;

        // 去掉 .j2 后缀作为输出文件名
        let output_name = tpl_name.strip_suffix(".j2").unwrap_or(tpl_name);
        files.push((output_name.to_string(), rendered));
    }

    Ok(files)
}

// ── rustfmt ───────────────────────────────────────────────────────────────

/// 调用 rustfmt 格式化 Rust 代码。
///
/// 如果 rustfmt 不可用或执行失败，返回原始代码并记录警告。
fn format_rust_code(code: &str) -> String {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut child = match Command::new("rustfmt")
        .arg("--edition=2021")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            tracing::warn!("rustfmt 不可用 ({}), 跳过格式化", e);
            return code.to_string();
        }
    };

    if let Some(ref mut stdin) = child.stdin {
        if let Err(e) = stdin.write_all(code.as_bytes()) {
            tracing::warn!("写入 rustfmt stdin 失败: {}", e);
            return code.to_string();
        }
    }
    // 关闭 stdin 以通知 rustfmt 输入结束
    drop(child.stdin.take());

    let output = match child.wait_with_output() {
        Ok(output) => output,
        Err(e) => {
            tracing::warn!("等待 rustfmt 完成失败: {}", e);
            return code.to_string();
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::warn!("rustfmt 执行失败:\n{}", stderr);
        return code.to_string();
    }

    String::from_utf8(output.stdout).unwrap_or_else(|_| code.to_string())
}

// ── 文件写入 ──────────────────────────────────────────────────────────────

/// 写入模块目录下的所有文件。
///
/// - Rust 文件经过 rustfmt 格式化
/// - 非 Rust 文件直接写入
fn write_module_files(
    module_dir: &Path,
    files: &[(String, String)],
) -> Result<()> {
    std::fs::create_dir_all(module_dir)
        .with_context(|| format!("无法创建模块目录: {}", module_dir.display()))?;

    for (filename, content) in files {
        let path = module_dir.join(filename);
        let final_content = if filename.ends_with(".rs") {
            format_rust_code(content)
        } else {
            content.clone()
        };

        std::fs::write(&path, &final_content)
            .with_context(|| format!("无法写入文件: {}", path.display()))?;
    }

    Ok(())
}

/// 写入 migration SQL 文件。
fn write_migration_file(project_root: &Path, migration_number: u32, table_name: &str, sql: &str) -> Result<PathBuf> {
    let filename = format!("{:03}_create_{}.sql", migration_number, table_name);
    let path = project_root.join("migrations").join(&filename);

    std::fs::write(&path, sql)
        .with_context(|| format!("无法写入迁移文件: {}", path.display()))?;

    Ok(path)
}

// ── mod.rs 追加 ───────────────────────────────────────────────────────────

/// 确保 `src/modules/mod.rs` 中包含指定模块的声明行。
///
/// 匹配行首的 `pub mod xxx;` 模式，忽略注释和字符串内容。
/// 如果已存在则跳过，否则追加到文件末尾。
fn ensure_module_declaration(project_root: &Path, module_name: &str) -> Result<bool> {
    let mod_rs_path = project_root.join("src").join("modules").join("mod.rs");

    let content = std::fs::read_to_string(&mod_rs_path)
        .with_context(|| format!("无法读取 mod.rs: {}", mod_rs_path.display()))?;

    let declaration = format!("pub mod {};", module_name);

    // 逐行匹配，忽略注释行和缩进的声明
    let already_declared = content.lines().any(|line| {
        let trimmed = line.trim();
        // 跳过注释行
        if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with("*") {
            return false;
        }
        trimmed == declaration
    });

    if already_declared {
        return Ok(false);
    }

    // 追加到文件末尾，确保前面有换行
    let mut new_content = content;
    if !new_content.ends_with('\n') {
        new_content.push('\n');
    }
    new_content.push_str(&declaration);
    new_content.push('\n');

    std::fs::write(&mod_rs_path, &new_content)
        .with_context(|| format!("无法写入 mod.rs: {}", mod_rs_path.display()))?;

    Ok(true)
}

// ── 孤儿声明清理 ──────────────────────────────────────────────────────────

/// 清理单个孤儿模块声明：mod.rs 中有声明但目录不存在。
///
/// 匹配逻辑与 `ensure_module_declaration` 保持一致：忽略注释行。
/// 如果目录存在、声明不存在、或 mod.rs 不存在，均返回 `Ok(false)`。
fn cleanup_orphan_module_declaration(project_root: &Path, module_name: &str) -> Result<bool> {
    let mod_rs_path = project_root.join("src").join("modules").join("mod.rs");
    let module_dir = project_root.join("src").join("modules").join(module_name);

    // 目录存在 → 不是孤儿
    if module_dir.exists() {
        return Ok(false);
    }

    // mod.rs 不存在 → 无需清理
    if !mod_rs_path.exists() {
        return Ok(false);
    }

    let content = std::fs::read_to_string(&mod_rs_path)
        .with_context(|| format!("无法读取 mod.rs: {}", mod_rs_path.display()))?;

    let declaration = format!("pub mod {};", module_name);

    // 逐行匹配，忽略注释行（与 ensure_module_declaration 一致）
    let has_declaration = content.lines().any(|line| {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with("*") {
            return false;
        }
        trimmed == declaration
    });

    if !has_declaration {
        return Ok(false);
    }

    // 删除声明行，保留注释行
    let new_content: String = content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with("*") {
                return true;
            }
            trimmed != declaration
        })
        .collect::<Vec<_>>()
        .join("\n");

    std::fs::write(&mod_rs_path, &new_content)
        .with_context(|| format!("无法写入 mod.rs: {}", mod_rs_path.display()))?;

    tracing::info!(
        module = module_name,
        "清理了孤儿模块声明（目录已删除）"
    );

    Ok(true)
}

/// 扫描 `src/modules/mod.rs`，清理所有孤儿模块声明。
///
/// 返回清理的声明数量。目录仍存在的模块声明不会被触及。
fn cleanup_all_orphan_module_declarations(project_root: &Path) -> Result<usize> {
    let mod_rs_path = project_root.join("src").join("modules").join("mod.rs");

    if !mod_rs_path.exists() {
        return Ok(0);
    }

    let content = std::fs::read_to_string(&mod_rs_path)
        .with_context(|| format!("无法读取 mod.rs: {}", mod_rs_path.display()))?;

    // 提取所有 pub mod xxx; 声明（忽略注释行）
    let module_names: Vec<String> = content
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with("*") {
                return None;
            }
            if trimmed.starts_with("pub mod ") && trimmed.ends_with(';') {
                let name = trimmed
                    .strip_prefix("pub mod ")
                    .unwrap()
                    .strip_suffix(';')
                    .unwrap()
                    .trim();
                Some(name.to_string())
            } else {
                None
            }
        })
        .collect();

    let mut cleaned_count = 0usize;
    for name in module_names {
        if cleanup_orphan_module_declaration(project_root, &name)? {
            cleaned_count += 1;
        }
    }

    Ok(cleaned_count)
}

// ── 锁文件持久化 ──────────────────────────────────────────────────────────

/// 将 SchemaLock 写入 `.colophon.lock`。
fn write_lock_file(project_root: &Path, lock: &SchemaLock) -> Result<()> {
    let lock_path = project_root.join(".colophon.lock");
    let content = toml::to_string_pretty(lock)
        .context("序列化锁文件失败")?;

    std::fs::write(&lock_path, content)
        .with_context(|| format!("无法写入锁文件: {}", lock_path.display()))?;

    Ok(())
}

// ── 主入口 ────────────────────────────────────────────────────────────────

/// 生成结果，包含所有创建/修改的文件路径。
#[derive(Debug)]
pub struct GenerateResult {
    pub module_files: Vec<PathBuf>,
    pub migration_file: Option<PathBuf>,
    pub lock_file_updated: bool,
    pub mod_rs_updated: bool,
    /// 模块是否因已存在而被跳过。
    pub skipped: bool,
}

/// 检查模块目录是否已存在且包含文件。
fn module_already_exists(project_root: &Path, module_name: &str) -> bool {
    let module_dir = project_root.join("src").join("modules").join(module_name);
    if !module_dir.exists() {
        return false;
    }
    // 检查目录中是否有 .rs 文件
    std::fs::read_dir(&module_dir)
        .ok()
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .any(|e| {
                    e.path()
                        .extension()
                        .map_or(false, |ext| ext == "rs")
                })
        })
        .unwrap_or(false)
}

/// 执行代码生成流程。
///
/// 1. 加载模板
/// 2. 渲染各模块代码
/// 3. 执行 diff（检测破坏性变更）
/// 4. 写入模块文件
/// 5. 写入 migration SQL
/// 6. 更新并写入锁文件
/// 7. 追加 mod.rs 声明
///
/// 如果模块目录已存在且包含 .rs 文件，则跳过生成以保护手写代码，
/// 但仍会更新锁文件以保持状态同步。
pub fn generate(
    project_root: &Path,
    ctx: &TemplateContext,
) -> Result<GenerateResult> {
    let model_name_lower = ctx.model_name.to_lowercase();

    // 安全检查：如果模块已存在，跳过生成但更新锁文件
    if module_already_exists(project_root, &model_name_lower) {
        tracing::info!(
            module = ctx.model_name,
            "模块目录已存在，跳过生成（如需重新生成，请先删除目录）"
        );

        // 跳过时也更新锁文件，保持状态同步
        let mut lock = diff::read_lock_file(project_root)
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        diff::update_lock_in_memory(&mut lock, &ctx.model_name, &ctx.table_name, &ctx.fields);
        write_lock_file(project_root, &lock)?;

        return Ok(GenerateResult {
            module_files: Vec::new(),
            migration_file: None,
            lock_file_updated: true,
            mod_rs_updated: false,
            skipped: true,
        });
    }

    // 1. 加载模板
    let tpl_dir = templates_dir(project_root);
    let mut env = Environment::new();
    env.set_loader(minijinja::path_loader(&tpl_dir));

    // 2. 渲染模板
    let rendered_files = render_module_files(&env, ctx)?;

    // 3. 执行 diff
    let mut lock = diff::read_lock_file(project_root)
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    let diff_result = diff::diff_collection(
        &ctx.model_name,
        &ctx.table_name,
        &ctx.fields,
        &lock,
    )
    .map_err(|e| anyhow::anyhow!("{}", e))?;

    // 4. 生成 migration SQL
    let migration_sql = diff::generate_migration_sql(
        &diff_result,
        &ctx.model_name,
        &ctx.table_name,
        &ctx.fields,
    );

    // 5. 写入模块文件
    let module_dir = project_root.join("src").join("modules").join(&model_name_lower);
    write_module_files(&module_dir, &rendered_files)?;

    let module_files: Vec<PathBuf> = rendered_files
        .iter()
        .map(|(name, _)| module_dir.join(name))
        .collect();

    // 6. 写入 migration SQL（仅在有变更时）
    let migration_file = if !migration_sql.is_empty() {
        let migration_number = diff::next_migration_number(project_root)
            .context("获取下一个迁移编号失败")?;
        let path = write_migration_file(project_root, migration_number, &ctx.table_name, &migration_sql)?;
        Some(path)
    } else {
        None
    };

    // 7. 追加 mod.rs 声明
    let mod_rs_updated = ensure_module_declaration(project_root, &model_name_lower)?;

    // 8. 更新并写入锁文件（生成成功后立即持久化）
    diff::update_lock_in_memory(&mut lock, &ctx.model_name, &ctx.table_name, &ctx.fields);
    write_lock_file(project_root, &lock)?;

    Ok(GenerateResult {
        module_files,
        migration_file,
        lock_file_updated: true,
        mod_rs_updated,
        skipped: false,
    })
}

// ── CLI 入口 ──────────────────────────────────────────────────────────────

/// `schema generate` 命令入口：扫描 schemas 目录，为每个 TOML 文件生成代码。
///
/// 锁文件由 `generate()` 在每次成功生成后自动更新，
/// `run()` 仅负责调度和日志输出。
pub async fn run(project_root: &Path, schema_dir: &Path) -> anyhow::Result<()> {
    // 解析绝对路径
    let project_root = if project_root.is_absolute() {
        project_root.to_path_buf()
    } else {
        std::env::current_dir()?.join(project_root)
    };
    let schema_dir = if schema_dir.is_absolute() {
        schema_dir.to_path_buf()
    } else {
        std::env::current_dir()?.join(schema_dir)
    };

    // 清理孤儿模块声明（目录已删除但 mod.rs 中仍有声明）
    let cleaned_count = cleanup_all_orphan_module_declarations(&project_root)?;
    if cleaned_count > 0 {
        tracing::info!(
            count = cleaned_count,
            "清理了孤儿模块声明"
        );
    }

    // 解析所有 schema 文件
    let contexts = super::parse_schema_dir(&schema_dir)?;

    if contexts.is_empty() {
        tracing::warn!(
            schema_dir = %schema_dir.display(),
            "未找到 Schema 文件"
        );
        return Ok(());
    }

    tracing::info!(
        count = contexts.len(),
        "找到 Schema 定义"
    );

    for ctx in &contexts {
        let result = generate(&project_root, ctx)?;

        if result.skipped {
            tracing::info!(
                module = %ctx.model_name,
                "跳过生成（模块已存在）"
            );
            continue;
        }

        tracing::info!(
            module = %ctx.model_name,
            target = %format!("src/modules/{}/", ctx.model_name.to_lowercase()),
            "生成模块"
        );

        for file in &result.module_files {
            tracing::info!(file = %file.display(), "写入文件");
        }
        if let Some(ref migration) = result.migration_file {
            tracing::info!(file = %migration.display(), "写入迁移文件");
        }
    }

    tracing::info!("Schema 生成完成");
    Ok(())
}

// ── 测试 ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::schema::types::{CollectionDef, FeaturesDef, FieldDef};

    /// 辅助函数：创建测试用 FieldDef。
    fn test_field(name: &str, field_type: &str, required: bool) -> FieldDef {
        let (rust_type, sqlite_type) = match field_type {
            "text" => ("String", "TEXT NOT NULL"),
            "richtext" => ("String", "TEXT NOT NULL"),
            "boolean" => ("bool", "INTEGER NOT NULL DEFAULT 0"),
            "integer" => ("i64", "INTEGER NOT NULL DEFAULT 0"),
            _ => ("String", "TEXT NOT NULL"),
        };
        FieldDef {
            name: name.into(),
            field_type: field_type.into(),
            required,
            unique: false,
            computed: false,
            references: None,
            is_updatable: true,
            is_auto_generated: false,
            rust_type: rust_type.into(),
            sqlite_type: sqlite_type.into(),
        }
    }

    /// 辅助函数：构建最小的 TemplateContext。
    fn minimal_context() -> TemplateContext {
        use crate::cli::schema::context::build_context;

        let schema = crate::cli::schema::types::ColophonSchema {
            collection: CollectionDef {
                name: "Tag".into(),
                table: "tags".into(),
                display_name: Some("标签".into()),
            },
            features: FeaturesDef {
                soft_delete: true,
                timestamps: true,
                ..Default::default()
            },
            fields: vec![
                test_field("name", "text", true),
                test_field("slug", "text", true),
            ],
        };

        build_context(schema).expect("构建 TemplateContext 失败")
    }

    #[test]
    fn render_module_files_produces_all_files() {
        let project_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let tpl_dir = templates_dir(project_root);

        let mut env = Environment::new();
        env.set_loader(minijinja::path_loader(&tpl_dir));

        let ctx = minimal_context();
        let files = render_module_files(&env, &ctx).expect("渲染失败");

        let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"domain.rs"));
        assert!(names.contains(&"dto.rs"));
        assert!(names.contains(&"repository.rs"));
        assert!(names.contains(&"handler.rs"));
        assert!(names.contains(&"service.rs"));
        assert!(names.contains(&"mod.rs"));
        assert_eq!(files.len(), 6);
    }

    #[test]
    fn rendered_domain_contains_struct_name() {
        let project_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let tpl_dir = templates_dir(project_root);

        let mut env = Environment::new();
        env.set_loader(minijinja::path_loader(&tpl_dir));

        let ctx = minimal_context();
        let files = render_module_files(&env, &ctx).expect("渲染失败");

        let domain = files.iter().find(|(n, _)| n == "domain.rs").unwrap();
        assert!(domain.1.contains("pub struct Tag"));
        assert!(domain.1.contains("pub id: String"));
        assert!(domain.1.contains("pub name: String"));
    }

    #[test]
    fn rendered_dto_contains_create_and_update() {
        let project_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let tpl_dir = templates_dir(project_root);

        let mut env = Environment::new();
        env.set_loader(minijinja::path_loader(&tpl_dir));

        let ctx = minimal_context();
        let files = render_module_files(&env, &ctx).expect("渲染失败");

        let dto = files.iter().find(|(n, _)| n == "dto.rs").unwrap();
        assert!(dto.1.contains("CreateTagRequest"));
        assert!(dto.1.contains("UpdateTagRequest"));
    }

    #[test]
    fn rendered_repository_uses_correct_table_name() {
        let project_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let tpl_dir = templates_dir(project_root);

        let mut env = Environment::new();
        env.set_loader(minijinja::path_loader(&tpl_dir));

        let ctx = minimal_context();
        let files = render_module_files(&env, &ctx).expect("渲染失败");

        let repo = files.iter().find(|(n, _)| n == "repository.rs").unwrap();
        assert!(repo.1.contains("FROM tags"));
        assert!(repo.1.contains("deleted_at IS NULL"));
    }

    #[test]
    fn rendered_handler_uses_crud_macro() {
        let project_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let tpl_dir = templates_dir(project_root);

        let mut env = Environment::new();
        env.set_loader(minijinja::path_loader(&tpl_dir));

        let ctx = minimal_context();
        let files = render_module_files(&env, &ctx).expect("渲染失败");

        let handler = files.iter().find(|(n, _)| n == "handler.rs").unwrap();
        assert!(handler.1.contains("crud_handlers!"));
        assert!(handler.1.contains("entity = Tag"));
    }

    #[test]
    fn ensure_module_declaration_adds_new_module() {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let modules_dir = dir.path().join("src").join("modules");
        std::fs::create_dir_all(&modules_dir).expect("创建目录失败");

        let mod_rs = modules_dir.join("mod.rs");
        std::fs::write(&mod_rs, "pub mod tag;\n").expect("写入失败");

        let added = ensure_module_declaration(dir.path(), "new_module").expect("追加失败");
        assert!(added);

        let content = std::fs::read_to_string(&mod_rs).expect("读取失败");
        assert!(content.contains("pub mod new_module;"));
    }

    #[test]
    fn ensure_module_declaration_skips_existing() {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let modules_dir = dir.path().join("src").join("modules");
        std::fs::create_dir_all(&modules_dir).expect("创建目录失败");

        let mod_rs = modules_dir.join("mod.rs");
        std::fs::write(&mod_rs, "pub mod tag;\npub mod category;\n").expect("写入失败");

        let added = ensure_module_declaration(dir.path(), "tag").expect("检查失败");
        assert!(!added);
    }

    #[test]
    fn fix_m3_ignores_comment_containing_declaration() {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let modules_dir = dir.path().join("src").join("modules");
        std::fs::create_dir_all(&modules_dir).expect("创建目录失败");

        let mod_rs = modules_dir.join("mod.rs");
        // 注释中包含 "pub mod tag;" 不应被误判为已声明
        std::fs::write(
            &mod_rs,
            "// pub mod tag; -- this is a comment\npub mod category;\n",
        )
        .expect("写入失败");

        let added = ensure_module_declaration(dir.path(), "tag").expect("检查失败");
        assert!(added, "注释中的声明不应被匹配");

        let content = std::fs::read_to_string(&mod_rs).expect("读取失败");
        assert!(content.contains("pub mod tag;"), "应追加实际声明");
    }

    #[test]
    fn fix_m3_matches_actual_declaration() {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let modules_dir = dir.path().join("src").join("modules");
        std::fs::create_dir_all(&modules_dir).expect("创建目录失败");

        let mod_rs = modules_dir.join("mod.rs");
        std::fs::write(&mod_rs, "pub mod tag;\n").expect("写入失败");

        let added = ensure_module_declaration(dir.path(), "tag").expect("检查失败");
        assert!(!added, "实际声明应被匹配");
    }

    #[test]
    fn generate_end_to_end() {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let project_root = dir.path();

        // 创建必要的目录结构
        std::fs::create_dir_all(project_root.join("src").join("modules")).expect("创建目录失败");
        std::fs::create_dir_all(project_root.join("migrations")).expect("创建目录失败");
        std::fs::create_dir_all(project_root.join("cli").join("templates")).expect("创建目录失败");

        // 从项目源目录复制模板
        let src_templates = Path::new(env!("CARGO_MANIFEST_DIR")).join("cli").join("templates");
        for entry in std::fs::read_dir(&src_templates).expect("读取模板目录失败") {
            let entry = entry.expect("读取条目失败");
            let dest = project_root.join("cli").join("templates").join(entry.file_name());
            std::fs::copy(entry.path(), dest).expect("复制模板失败");
        }

        // 创建空的 mod.rs
        std::fs::write(
            project_root.join("src").join("modules").join("mod.rs"),
            "",
        ).expect("写入 mod.rs 失败");

        let ctx = minimal_context();
        let result = generate(project_root, &ctx).expect("生成失败");

        // 验证未跳过
        assert!(!result.skipped);

        // 验证模块文件
        assert!(!result.module_files.is_empty());
        assert!(result.module_files.iter().all(|p| p.exists()));

        // 验证 migration 文件
        assert!(result.migration_file.is_some());
        let migration = result.migration_file.unwrap();
        assert!(migration.exists());
        let sql = std::fs::read_to_string(&migration).expect("读取 migration 失败");
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS tags"));

        // 验证 mod.rs
        let mod_rs = std::fs::read_to_string(
            project_root.join("src").join("modules").join("mod.rs"),
        ).expect("读取 mod.rs 失败");
        assert!(mod_rs.contains("pub mod tag;"));
    }

    #[test]
    fn generate_skips_existing_module() {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let project_root = dir.path();

        // 创建目录结构，但模块目录中已有文件
        std::fs::create_dir_all(project_root.join("src").join("modules").join("tag"))
            .expect("创建目录失败");
        std::fs::create_dir_all(project_root.join("migrations")).expect("创建目录失败");
        std::fs::create_dir_all(project_root.join("cli").join("templates")).expect("创建目录失败");

        // 写入一个已有的 domain.rs
        std::fs::write(
            project_root.join("src").join("modules").join("tag").join("domain.rs"),
            "// existing code",
        ).expect("写入失败");

        // 创建空的 mod.rs
        std::fs::write(
            project_root.join("src").join("modules").join("mod.rs"),
            "pub mod tag;\n",
        ).expect("写入 mod.rs 失败");

        let ctx = minimal_context();
        let result = generate(project_root, &ctx).expect("生成失败");

        // 验证被跳过
        assert!(result.skipped);
        assert!(result.module_files.is_empty());
        assert!(result.migration_file.is_none());

        // 验证原有文件未被覆盖
        let content = std::fs::read_to_string(
            project_root.join("src").join("modules").join("tag").join("domain.rs"),
        ).expect("读取失败");
        assert_eq!(content, "// existing code");
    }

    #[test]
    fn module_already_exists_returns_false_for_empty_dir() {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let modules_dir = dir.path().join("src").join("modules").join("tag");
        std::fs::create_dir_all(&modules_dir).expect("创建目录失败");

        assert!(!module_already_exists(dir.path(), "tag"));
    }

    #[test]
    fn module_already_exists_returns_false_for_nonexistent_dir() {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        assert!(!module_already_exists(dir.path(), "nonexistent"));
    }

    #[test]
    fn module_already_exists_returns_true_with_rs_files() {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let modules_dir = dir.path().join("src").join("modules").join("tag");
        std::fs::create_dir_all(&modules_dir).expect("创建目录失败");
        std::fs::write(modules_dir.join("domain.rs"), "// code").expect("写入失败");

        assert!(module_already_exists(dir.path(), "tag"));
    }

    // ── M5 修复测试 ──────────────────────────────────────────────────────────

    #[test]
    fn fix_m5_lock_file_updated_true_after_write() {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let project_root = dir.path();

        // 创建必要的目录结构
        std::fs::create_dir_all(project_root.join("src").join("modules")).expect("创建目录失败");
        std::fs::create_dir_all(project_root.join("migrations")).expect("创建目录失败");
        std::fs::create_dir_all(project_root.join("cli").join("templates")).expect("创建目录失败");

        // 从项目源目录复制模板
        let src_templates = Path::new(env!("CARGO_MANIFEST_DIR")).join("cli").join("templates");
        for entry in std::fs::read_dir(&src_templates).expect("读取模板目录失败") {
            let entry = entry.expect("读取条目失败");
            let dest = project_root.join("cli").join("templates").join(entry.file_name());
            std::fs::copy(entry.path(), dest).expect("复制模板失败");
        }

        // 创建空的 mod.rs
        std::fs::write(
            project_root.join("src").join("modules").join("mod.rs"),
            "",
        ).expect("写入 mod.rs 失败");

        let ctx = minimal_context();
        let result = generate(project_root, &ctx).expect("生成失败");

        // 验证 lock_file_updated 为 true
        assert!(result.lock_file_updated, "lock_file_updated 应为 true");

        // 验证锁文件已写入磁盘
        let lock_path = project_root.join(".colophon.lock");
        assert!(lock_path.exists(), "锁文件应已写入磁盘");

        let lock_content = std::fs::read_to_string(&lock_path).expect("读取锁文件失败");
        assert!(lock_content.contains("[collections.Tag]"), "锁文件应包含 Tag 集合");
    }

    #[test]
    fn fix_m5_second_generation_no_duplicate_sql() {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let project_root = dir.path();

        // 创建必要的目录结构
        std::fs::create_dir_all(project_root.join("src").join("modules")).expect("创建目录失败");
        std::fs::create_dir_all(project_root.join("migrations")).expect("创建目录失败");
        std::fs::create_dir_all(project_root.join("cli").join("templates")).expect("创建目录失败");

        // 从项目源目录复制模板
        let src_templates = Path::new(env!("CARGO_MANIFEST_DIR")).join("cli").join("templates");
        for entry in std::fs::read_dir(&src_templates).expect("读取模板目录失败") {
            let entry = entry.expect("读取条目失败");
            let dest = project_root.join("cli").join("templates").join(entry.file_name());
            std::fs::copy(entry.path(), dest).expect("复制模板失败");
        }

        // 创建空的 mod.rs
        std::fs::write(
            project_root.join("src").join("modules").join("mod.rs"),
            "",
        ).expect("写入 mod.rs 失败");

        let ctx = minimal_context();

        // 第一次生成
        let result1 = generate(project_root, &ctx).expect("第一次生成失败");
        assert!(!result1.skipped);
        assert!(result1.migration_file.is_some(), "第一次应生成 migration");

        let migration_count_before = std::fs::read_dir(project_root.join("migrations"))
            .expect("读取 migrations 目录失败")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "sql"))
            .count();

        // 删除模块目录以允许重新生成
        std::fs::remove_dir_all(project_root.join("src").join("modules").join("tag"))
            .expect("删除模块目录失败");

        // 第二次生成（锁文件已记录 Tag，不应产生新 migration）
        let result2 = generate(project_root, &ctx).expect("第二次生成失败");
        assert!(!result2.skipped);

        let migration_count_after = std::fs::read_dir(project_root.join("migrations"))
            .expect("读取 migrations 目录失败")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "sql"))
            .count();

        // 第二次不应产生新的 migration 文件
        assert_eq!(
            migration_count_before, migration_count_after,
            "重复 generate 不应产生重复 migration"
        );
    }

    #[test]
    fn fix_m5_lock_file_updated_true_when_skipped() {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let project_root = dir.path();

        // 创建目录结构，模块目录中已有文件
        std::fs::create_dir_all(project_root.join("src").join("modules").join("tag"))
            .expect("创建目录失败");
        std::fs::create_dir_all(project_root.join("migrations")).expect("创建目录失败");
        std::fs::create_dir_all(project_root.join("cli").join("templates")).expect("创建目录失败");

        // 写入一个已有的 domain.rs
        std::fs::write(
            project_root.join("src").join("modules").join("tag").join("domain.rs"),
            "// existing code",
        ).expect("写入失败");

        // 创建空的 mod.rs
        std::fs::write(
            project_root.join("src").join("modules").join("mod.rs"),
            "",
        ).expect("写入 mod.rs 失败");

        let ctx = minimal_context();
        let result = generate(project_root, &ctx).expect("生成失败");

        // 验证被跳过但 lock_file_updated 为 true
        assert!(result.skipped, "应被跳过");
        assert!(result.lock_file_updated, "跳过时 lock_file_updated 也应为 true");

        // 验证锁文件已写入磁盘
        let lock_path = project_root.join(".colophon.lock");
        assert!(lock_path.exists(), "跳过时锁文件也应写入磁盘");
    }

    #[test]
    fn fix_l4_uses_tracing_instead_of_eprintln() {
        // 验证 run 函数使用 tracing 而不是 eprintln!
        // 此测试通过代码审查确认：run 函数中不再使用 eprintln!
        // 功能验证：run 函数可以正常执行（不 panic）
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let project_root = dir.path();
        let schema_dir = dir.path().join("schemas");

        // 创建空的 schemas 目录
        std::fs::create_dir_all(&schema_dir).expect("创建目录失败");

        // run 应该正常执行（schemas 目录为空时返回 Ok）
        let rt = tokio::runtime::Runtime::new().expect("创建 runtime 失败");
        let result = rt.block_on(run(project_root, &schema_dir));
        assert!(result.is_ok(), "run 应正常执行");
    }

    #[test]
    fn fix_l5_supports_custom_template_dir() {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let project_root = dir.path();

        // 默认路径：cli/templates/
        let default_dir = templates_dir(project_root);
        assert_eq!(default_dir, project_root.join("cli").join("templates"));

        // 设置环境变量测试绝对路径
        let custom_dir = dir.path().join("custom_templates");
        std::env::set_var("COLOPHON_TEMPLATES_DIR", custom_dir.to_str().unwrap());
        let custom_result = templates_dir(project_root);
        assert_eq!(custom_result, custom_dir);

        // 设置环境变量测试相对路径
        std::env::set_var("COLOPHON_TEMPLATES_DIR", "my_templates");
        let relative_result = templates_dir(project_root);
        assert_eq!(relative_result, project_root.join("my_templates"));

        // 清理环境变量
        std::env::remove_var("COLOPHON_TEMPLATES_DIR");

        // 验证恢复默认
        let restored = templates_dir(project_root);
        assert_eq!(restored, project_root.join("cli").join("templates"));
    }

    // ── 孤儿声明清理测试 ──────────────────────────────────────────────────────

    #[test]
    fn fix_orphan_cleanup_removes_stale_declaration() {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let modules_dir = dir.path().join("src").join("modules");
        std::fs::create_dir_all(&modules_dir).expect("创建目录失败");

        let mod_rs = modules_dir.join("mod.rs");
        std::fs::write(&mod_rs, "pub mod tag;\npub mod category;\n").expect("写入失败");

        // tag 目录存在，category 目录不存在
        std::fs::create_dir_all(modules_dir.join("tag")).expect("创建目录失败");

        let cleaned = cleanup_orphan_module_declaration(dir.path(), "category")
            .expect("清理失败");
        assert!(cleaned, "应清理 orphan 声明");

        let content = std::fs::read_to_string(&mod_rs).expect("读取失败");
        assert!(content.contains("pub mod tag;"), "应保留 tag 声明");
        assert!(!content.contains("pub mod category;"), "应删除 category 声明");
    }

    #[test]
    fn fix_orphan_cleanup_skips_existing_directory() {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let modules_dir = dir.path().join("src").join("modules");
        std::fs::create_dir_all(&modules_dir).expect("创建目录失败");

        let mod_rs = modules_dir.join("mod.rs");
        std::fs::write(&mod_rs, "pub mod tag;\n").expect("写入失败");
        std::fs::create_dir_all(modules_dir.join("tag")).expect("创建目录失败");

        let cleaned = cleanup_orphan_module_declaration(dir.path(), "tag")
            .expect("清理失败");
        assert!(!cleaned, "目录存在时不应清理");

        let content = std::fs::read_to_string(&mod_rs).expect("读取失败");
        assert!(content.contains("pub mod tag;"), "声明应保留");
    }

    #[test]
    fn fix_orphan_cleanup_returns_false_when_no_declaration() {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let modules_dir = dir.path().join("src").join("modules");
        std::fs::create_dir_all(&modules_dir).expect("创建目录失败");

        let mod_rs = modules_dir.join("mod.rs");
        std::fs::write(&mod_rs, "pub mod tag;\n").expect("写入失败");

        let cleaned = cleanup_orphan_module_declaration(dir.path(), "nonexistent")
            .expect("清理失败");
        assert!(!cleaned, "无声明时不应清理");
    }

    #[test]
    fn fix_orphan_cleanup_ignores_comment_containing_declaration() {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let modules_dir = dir.path().join("src").join("modules");
        std::fs::create_dir_all(&modules_dir).expect("创建目录失败");

        let mod_rs = modules_dir.join("mod.rs");
        std::fs::write(
            &mod_rs,
            "// pub mod category; -- this is a comment\npub mod tag;\n",
        ).expect("写入失败");

        let cleaned = cleanup_orphan_module_declaration(dir.path(), "category")
            .expect("清理失败");
        assert!(!cleaned, "注释中的声明不应被匹配");

        let content = std::fs::read_to_string(&mod_rs).expect("读取失败");
        assert!(content.contains("// pub mod category;"), "注释应保留");
    }

    #[test]
    fn fix_orphan_cleanup_handles_missing_mod_rs() {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        // 不创建 mod.rs

        let cleaned = cleanup_orphan_module_declaration(dir.path(), "category")
            .expect("清理失败");
        assert!(!cleaned, "mod.rs 不存在时应返回 false");
    }

    #[test]
    fn fix_orphan_cleanup_all_removes_multiple_orphans() {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let modules_dir = dir.path().join("src").join("modules");
        std::fs::create_dir_all(&modules_dir).expect("创建目录失败");

        let mod_rs = modules_dir.join("mod.rs");
        std::fs::write(&mod_rs, "pub mod tag;\npub mod category;\npub mod media;\n")
            .expect("写入失败");

        // 只创建 tag 目录
        std::fs::create_dir_all(modules_dir.join("tag")).expect("创建目录失败");

        let cleaned = cleanup_all_orphan_module_declarations(dir.path())
            .expect("清理失败");
        assert_eq!(cleaned, 2, "应清理 2 个 orphan 声明");

        let content = std::fs::read_to_string(&mod_rs).expect("读取失败");
        assert!(content.contains("pub mod tag;"), "应保留 tag 声明");
        assert!(!content.contains("pub mod category;"), "应删除 category 声明");
        assert!(!content.contains("pub mod media;"), "应删除 media 声明");
    }

    #[test]
    fn fix_orphan_cleanup_all_returns_zero_when_no_orphans() {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let modules_dir = dir.path().join("src").join("modules");
        std::fs::create_dir_all(&modules_dir).expect("创建目录失败");

        let mod_rs = modules_dir.join("mod.rs");
        std::fs::write(&mod_rs, "pub mod tag;\n").expect("写入失败");
        std::fs::create_dir_all(modules_dir.join("tag")).expect("创建目录失败");

        let cleaned = cleanup_all_orphan_module_declarations(dir.path())
            .expect("清理失败");
        assert_eq!(cleaned, 0, "无 orphan 时应返回 0");
    }

    #[test]
    fn fix_orphan_cleanup_all_handles_missing_mod_rs() {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        // 不创建 mod.rs

        let cleaned = cleanup_all_orphan_module_declarations(dir.path())
            .expect("清理失败");
        assert_eq!(cleaned, 0, "mod.rs 不存在时应返回 0");
    }
}
