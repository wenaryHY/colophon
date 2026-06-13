use std::{
    collections::HashSet,
    io::{Cursor, Read},
    sync::Arc,
};

use anyhow::Context;
use sha2::{Digest, Sha256};
use sqlx::{Row, SqliteConnection};
use tokio::fs;
use zip::ZipArchive;

use crate::{
    infra::backup::{BackupStorageBackend, LocalBackupStorage},
    shared::error::{AppError, AppResult},
    state::AppState,
};

use super::{
    domain::{BackupStatus, RestoreStep},
    dto::RestoreProgressResponse,
    repository,
};

const BACKUP_ARCHIVE_NAME: &str = "backup.zip";
const BACKUP_DB_ENTRY: &str = "database/colophon.db";
const SYSTEM_SKIP_TABLES: &[&str] = &[
    "backups",
    "backup_schedules",
    "theme_configs",
    "_sqlx_migrations",
];

fn local_backend(state: &AppState) -> AppResult<LocalBackupStorage> {
    Ok(LocalBackupStorage::new(state.backup_root_dir()))
}

fn hash_bytes(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

fn is_safe_sqlite_ident(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn quote_sqlite_ident(name: &str) -> String {
    let escaped = name.replace('"', "\"\"");
    format!("\"{}\"", escaped)
}

fn quote_sqlite_literal(value: &str) -> String {
    value.replace('\'', "''")
}

fn extract_archive(bytes: &[u8]) -> AppResult<(Vec<u8>, String)> {
    let reader = Cursor::new(bytes);
    let mut archive = ZipArchive::new(reader)
        .map_err(|e| AppError::BadRequest(format!("无法读取备份压缩包: {e}")))?;

    let mut db_bytes = Vec::new();
    {
        let mut db_file = archive
            .by_name(BACKUP_DB_ENTRY)
            .map_err(|_| AppError::BadRequest("备份包缺少数据库文件".into()))?;
        db_file.read_to_end(&mut db_bytes)?;
    }

    let mut manifest = String::new();
    {
        let mut manifest_file = archive
            .by_name("manifest.json")
            .map_err(|_| AppError::BadRequest("备份包缺少 manifest.json".into()))?;
        manifest_file.read_to_string(&mut manifest)?;
    }

    Ok((db_bytes, manifest))
}

async fn validate_sqlite_image_bytes(state: &AppState, db_bytes: &[u8]) -> AppResult<()> {
    let backup_dir = state.backup_root_dir();
    fs::create_dir_all(&backup_dir).await?;
    let tmp_path = backup_dir.join(format!("validate-merge-{}.db", uuid::Uuid::new_v4()));
    fs::write(&tmp_path, db_bytes).await?;

    let tmp_literal = quote_sqlite_literal(tmp_path.to_string_lossy().as_ref());
    let attach_sql = format!("ATTACH DATABASE '{}' AS check_db", tmp_literal);
    let mut conn = state.pool.acquire().await?;

    let validation_result = async {
        sqlx::query(&attach_sql).execute(&mut *conn).await?;
        let check_result: String = sqlx::query_scalar("PRAGMA check_db.integrity_check")
            .fetch_one(&mut *conn)
            .await?;
        sqlx::query("DETACH DATABASE check_db")
            .execute(&mut *conn)
            .await?;
        Ok::<String, sqlx::Error>(check_result)
    }
    .await;

    let _ = fs::remove_file(&tmp_path).await;

    let check_result = validation_result.map_err(|e| {
        AppError::BadRequest(format!("备份数据库镜像不可用（完整性检查失败）: {}", e))
    })?;
    if check_result.to_lowercase() != "ok" {
        return Err(AppError::BadRequest(format!(
            "备份数据库镜像损坏，integrity_check={}",
            check_result
        )));
    }

    Ok(())
}

async fn table_names(conn: &mut SqliteConnection, schema: &str) -> AppResult<Vec<String>> {
    let sql = format!(
        "SELECT name
         FROM pragma_table_list('{}')
         WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
        quote_sqlite_literal(schema)
    );
    let rows = sqlx::query(&sql).fetch_all(conn).await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(row.try_get::<String, _>("name")?);
    }
    Ok(out)
}

async fn table_columns(
    conn: &mut SqliteConnection,
    schema: &str,
    table: &str,
) -> AppResult<Vec<String>> {
    if schema != "main" && schema != "restore_db" {
        return Err(AppError::BadRequest(
            "invalid sqlite schema for merge".into(),
        ));
    }
    let sql = format!(
        "PRAGMA {}.table_info('{}')",
        schema,
        quote_sqlite_literal(table)
    );
    let rows = sqlx::query(&sql).fetch_all(conn).await?;
    let mut cols = Vec::with_capacity(rows.len());
    for row in rows {
        cols.push(row.try_get::<String, _>("name")?);
    }
    Ok(cols)
}

fn intersection_in_order(primary: &[String], secondary: &HashSet<String>) -> Vec<String> {
    primary
        .iter()
        .filter(|name| secondary.contains(name.as_str()))
        .cloned()
        .collect()
}

fn has_column(cols: &[String], column: &str) -> bool {
    cols.iter().any(|c| c == column)
}

async fn merge_upsert_table(
    conn: &mut SqliteConnection,
    table: &str,
    pk_col: &str,
) -> AppResult<()> {
    let main_columns = table_columns(conn, "main", table).await?;
    let restore_columns = table_columns(conn, "restore_db", table).await?;
    let restore_columns_set: HashSet<String> = restore_columns.into_iter().collect();
    let columns = intersection_in_order(&main_columns, &restore_columns_set);

    if !has_column(&columns, pk_col) || !has_column(&columns, "updated_at") {
        return Ok(());
    }

    if columns.iter().any(|c| !is_safe_sqlite_ident(c)) {
        tracing::warn!(
            module = "backup",
            event = "merge_skip_unsafe_columns",
            table = %table,
            "skipping table with non-alphanumeric column names during merge restore"
        );
        return Ok(());
    }

    let quoted_table = quote_sqlite_ident(table);
    let column_list = columns
        .iter()
        .map(|c| quote_sqlite_ident(c))
        .collect::<Vec<_>>()
        .join(", ");
    let assignment_list = columns
        .iter()
        .filter(|c| c.as_str() != pk_col)
        .map(|c| {
            let q = quote_sqlite_ident(c);
            format!("{q} = excluded.{q}")
        })
        .collect::<Vec<_>>()
        .join(", ");

    if assignment_list.is_empty() {
        return Ok(());
    }

    let pk = quote_sqlite_ident(pk_col);
    let sql = format!(
        "INSERT INTO {table} ({columns})
         SELECT {columns} FROM restore_db.{table}
         ON CONFLICT({pk}) DO UPDATE
         SET {assignments}
         WHERE COALESCE(excluded.\"updated_at\", '') > COALESCE({table}.\"updated_at\", '')",
        table = quoted_table,
        columns = column_list,
        pk = pk,
        assignments = assignment_list
    );
    sqlx::query(&sql).execute(conn).await?;
    Ok(())
}

async fn merge_post_tags(conn: &mut SqliteConnection) -> AppResult<()> {
    let sql = r#"
        INSERT INTO post_tags (post_id, tag_id)
        SELECT r.post_id, r.tag_id
        FROM restore_db.post_tags r
        LEFT JOIN post_tags m ON m.post_id = r.post_id AND m.tag_id = r.tag_id
        JOIN posts p ON p.id = r.post_id
        JOIN tags t ON t.id = r.tag_id
        WHERE m.post_id IS NULL
          AND (p.deleted_at IS NULL)
          AND (t.deleted_at IS NULL)
    "#;
    sqlx::query(sql).execute(conn).await?;
    Ok(())
}

async fn merge_database(state: &AppState, db_bytes: &[u8]) -> AppResult<()> {
    let backup_dir = state.backup_root_dir();
    fs::create_dir_all(&backup_dir).await?;
    let restore_path = backup_dir.join(format!("merge-restore-{}.db", uuid::Uuid::new_v4()));
    fs::write(&restore_path, db_bytes).await?;

    let restore_path_str = restore_path
        .to_str()
        .ok_or_else(|| AppError::Anyhow(anyhow::anyhow!("restore path is not valid UTF-8")))?;

    let mut conn = state.pool.acquire().await?;
    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(&mut *conn)
        .await?;
    sqlx::query("PRAGMA defer_foreign_keys = ON")
        .execute(&mut *conn)
        .await?;
    sqlx::query("ATTACH DATABASE ? AS restore_db")
        .bind(restore_path_str)
        .execute(&mut *conn)
        .await?;

    let merge_result = async {
        sqlx::query("BEGIN IMMEDIATE").execute(&mut *conn).await?;

        let main_tables = table_names(&mut conn, "main").await?;
        let restore_tables = table_names(&mut conn, "restore_db").await?;
        let restore_set: HashSet<String> = restore_tables.into_iter().collect();

        for table in main_tables {
            if SYSTEM_SKIP_TABLES.contains(&table.as_str()) {
                continue;
            }
            if !restore_set.contains(&table) {
                continue;
            }
            if !is_safe_sqlite_ident(&table) {
                tracing::warn!(
                    module = "backup",
                    event = "merge_skip_unsafe_table",
                    table = %table,
                    "skipping table with non-alphanumeric name during merge restore"
                );
                continue;
            }

            match table.as_str() {
                "post_tags" => {
                    merge_post_tags(&mut conn).await?;
                }
                "settings" => {
                    merge_upsert_table(&mut conn, &table, "key").await?;
                }
                _ => {
                    // Default strategy: if a table has id + updated_at, treat it as entity upsert table.
                    merge_upsert_table(&mut conn, &table, "id").await?;
                }
            }
        }

        sqlx::query("COMMIT").execute(&mut *conn).await?;
        Ok::<(), AppError>(())
    }
    .await;

    if merge_result.is_err() {
        let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
    }
    let _ = sqlx::query("DETACH DATABASE restore_db")
        .execute(&mut *conn)
        .await;
    let _ = sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&mut *conn)
        .await;
    let _ = fs::remove_file(&restore_path).await;

    merge_result
}

pub async fn merge_restore_backup(
    state: Arc<AppState>,
    backup_id: String,
) -> AppResult<Vec<RestoreProgressResponse>> {
    let backup = repository::get_backup(&state.pool, &backup_id)
        .await?
        .ok_or(AppError::NotFound)?;
    repository::update_backup_status(
        &state.pool,
        &backup.id,
        BackupStatus::Running.as_str(),
        None,
    )
    .await?;

    let result = async {
        let backend = local_backend(&state)?;
        let bytes = backend.read(&backup.id, BACKUP_ARCHIVE_NAME).await?;

        let mut progress = Vec::new();
        progress.push(RestoreProgressResponse {
            step: RestoreStep::Validate.as_str().to_string(),
            status: "completed".into(),
            message: "已完成备份包校验".into(),
        });

        let (db_bytes, manifest_raw) = extract_archive(&bytes)?;
        let expected_hash = hash_bytes(&db_bytes);
        let manifest_value: serde_json::Value = serde_json::from_str(&manifest_raw)
            .map_err(|e| AppError::BadRequest(format!("manifest.json 无法解析: {e}")))?;
        let manifest_hash = manifest_value["manifest_hash"].as_str().unwrap_or_default();
        if manifest_hash != expected_hash {
            return Err(AppError::BadRequest(
                "备份校验失败，manifest hash 不匹配".into(),
            ));
        }
        validate_sqlite_image_bytes(&state, &db_bytes).await?;
        progress.push(RestoreProgressResponse {
            step: RestoreStep::Extract.as_str().to_string(),
            status: "completed".into(),
            message: "已完成备份解压与镜像校验".into(),
        });

        merge_database(&state, &db_bytes)
            .await
            .with_context(|| format!("merge restore failed for backup_id={}", backup.id))?;
        progress.push(RestoreProgressResponse {
            step: RestoreStep::Merge.as_str().to_string(),
            status: "completed".into(),
            message: "已完成合并恢复".into(),
        });
        progress.push(RestoreProgressResponse {
            step: RestoreStep::ClearCache.as_str().to_string(),
            status: "completed".into(),
            message: "当前版本无额外缓存，已跳过缓存清理".into(),
        });

        Ok::<Vec<RestoreProgressResponse>, AppError>(progress)
    }
    .await;

    match &result {
        Ok(_) => {
            repository::update_backup_status(
                &state.pool,
                &backup.id,
                BackupStatus::Completed.as_str(),
                None,
            )
            .await?;
        }
        Err(err) => {
            let message = err.to_string();
            if let Err(status_err) = repository::update_backup_status(
                &state.pool,
                &backup.id,
                BackupStatus::Failed.as_str(),
                Some(&message),
            )
            .await
            {
                tracing::error!(
                    module = "backup",
                    backup_id = %backup.id,
                    error = %status_err,
                    "failed to update backup status to Failed after merge error"
                );
            }
        }
    }

    result
}
