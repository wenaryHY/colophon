use std::{
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
};

use anyhow::Context;
use chrono::{Datelike, Timelike, Utc, TimeZone};
use chrono_tz::Tz;
use cron::Schedule;
use sha2::{Digest, Sha256};
use sqlx::Row;
use tokio::fs;
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};

use crate::{
    infra::backup::{BackupStorageBackend, LocalBackupStorage},
    shared::error::{AppError, AppResult},
    state::AppState,
};

use super::{
    domain::{BackupProvider, BackupScheduleFrequency, BackupStatus, RestoreStep},
    dto::{
        BackupListResponse, BackupScheduleRequest, BackupScheduleResponse, RestoreProgressResponse,
    },
    repository,
};

const BACKUP_ARCHIVE_NAME: &str = "backup.zip";
const BACKUP_DB_ENTRY: &str = "database/colophon.db";
const BACKUP_MANIFEST_VERSION: i64 = 2;
const CURRENT_SCHEMA_VERSION: i64 = 7;

fn local_backend(state: &AppState) -> AppResult<LocalBackupStorage> {
    Ok(LocalBackupStorage::new(state.backup_root_dir()))
}

fn hash_bytes(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

fn quote_sqlite_literal(value: &str) -> String {
    value.replace('\'', "''")
}

async fn export_consistent_db_snapshot_strict(state: &AppState) -> AppResult<Vec<u8>> {
    let backups_dir = state.backup_root_dir();
    fs::create_dir_all(&backups_dir).await?;

    let snapshot_path = backups_dir.join(format!("snapshot-{}.db", uuid::Uuid::new_v4()));
    let snapshot_literal = quote_sqlite_literal(snapshot_path.to_string_lossy().as_ref());
    let vacuum_sql = format!("VACUUM INTO '{}'", snapshot_literal);

    let mut conn = state.pool.acquire().await?;
    let _ = sqlx::query("PRAGMA wal_checkpoint(FULL)")
        .execute(&mut *conn)
        .await;
    sqlx::query(&vacuum_sql)
        .execute(&mut *conn)
        .await
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("创建一致性数据库快照失败: {e}")))?;
    drop(conn);

    let bytes = fs::read(&snapshot_path)
        .await
        .with_context(|| format!("无法读取数据库快照文件: {}", snapshot_path.display()))?;
    let _ = fs::remove_file(&snapshot_path).await;
    Ok(bytes)
}

#[allow(dead_code)]
async fn export_consistent_db_snapshot(state: &AppState) -> AppResult<Vec<u8>> {
    let backups_dir = state.backup_root_dir();
    fs::create_dir_all(&backups_dir).await?;

    let snapshot_path = backups_dir.join(format!("snapshot-{}.db", uuid::Uuid::new_v4()));
    let snapshot_literal = quote_sqlite_literal(snapshot_path.to_string_lossy().as_ref());
    let vacuum_sql = format!("VACUUM INTO '{}'", snapshot_literal);

    let mut conn = state.pool.acquire().await?;

    let _ = sqlx::query("PRAGMA wal_checkpoint(FULL)")
        .execute(&mut *conn)
        .await;

    if let Err(err) = sqlx::query(&vacuum_sql).execute(&mut *conn).await {
        tracing::warn!(
            error = ?err,
            "failed to create consistent sqlite snapshot via VACUUM INTO, fallback to raw file read"
        );
        let _ = fs::remove_file(&snapshot_path).await;
        return Ok(fs::read(&state.db_path)
            .await
            .with_context(|| format!("无法读取数据库文件: {}", state.db_path.display()))?);
    }

    drop(conn);

    let bytes = fs::read(&snapshot_path)
        .await
        .with_context(|| format!("无法读取数据库快照文件: {}", snapshot_path.display()))?;
    let _ = fs::remove_file(&snapshot_path).await;
    Ok(bytes)
}

#[allow(dead_code)]
fn build_archive(db_bytes: &[u8], manifest_hash: &str, provider: &str) -> AppResult<Vec<u8>> {
    let cursor = Cursor::new(Vec::new());
    let mut writer = ZipWriter::new(cursor);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    writer
        .start_file(BACKUP_DB_ENTRY, options)
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("写入备份数据库条目失败: {e}")))?;
    writer.write_all(db_bytes)?;

    let manifest = serde_json::json!({
        "version": BACKUP_MANIFEST_VERSION,
        "schema_version": CURRENT_SCHEMA_VERSION,
        "app_version": env!("CARGO_PKG_VERSION"),
        "provider": provider,
        "created_at": Utc::now().to_rfc3339(),
        "manifest_hash": manifest_hash,
        "entries": [
            {
                "path": BACKUP_DB_ENTRY,
                "size": db_bytes.len()
            }
        ]
    });

    writer
        .start_file("manifest.json", options)
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("写入备份 manifest 失败: {e}")))?;
    writer.write_all(manifest.to_string().as_bytes())?;

    Ok(writer
        .finish()
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("完成备份压缩包失败: {e}")))?
        .into_inner())
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

async fn restore_database_file(state: &AppState, db_bytes: &[u8]) -> AppResult<PathBuf> {
    let bak_path = state.db_path.with_extension("db.bak");
    let restore_path = state.db_path.with_extension("db.restore");

    let _ = fs::remove_file(&bak_path).await;
    let _ = fs::remove_file(&restore_path).await;

    // Write new database to restore file
    if let Err(write_err) = fs::write(&restore_path, db_bytes).await {
        let _ = fs::remove_file(&restore_path).await;
        return Err(AppError::Anyhow(anyhow::anyhow!(
            "写入恢复数据库文件失败: {write_err}"
        )));
    }

    // Try to backup current database
    if fs::try_exists(&state.db_path).await? {
        if let Err(copy_err) = fs::copy(&state.db_path, &bak_path).await {
            tracing::warn!(error = ?copy_err, "failed to backup current db during restore");
            let _ = fs::remove_file(&restore_path).await;
            return Err(AppError::Anyhow(anyhow::anyhow!(
                "备份当前数据库失败: {copy_err}"
            )));
        }
    }

    // Try to replace database file with retries
    let mut last_err = None;
    for attempt in 0..5 {
        match fs::rename(&restore_path, &state.db_path).await {
            Ok(_) => {
                tracing::info!(
                    "database file replaced successfully on attempt {}",
                    attempt + 1
                );
                return Ok(bak_path);
            }
            Err(e) => {
                last_err = Some(e);
                if attempt < 4 {
                    tracing::warn!(
                        "attempt {} to replace db file failed, retrying...",
                        attempt + 1
                    );
                    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                }
            }
        }
    }

    tracing::warn!(error = ?last_err, "failed to replace db file, trying logical in-place restore");

    if let Err(logic_err) = restore_database_logically(state, &restore_path).await {
        tracing::error!(
            error = ?logic_err,
            "logical in-place restore failed, keeping restore file for manual recovery"
        );
        return Err(AppError::Anyhow(anyhow::anyhow!(
            "无法自动恢复数据库。文件替换失败且逻辑恢复失败，请停止服务后手动使用 {} 覆盖 {}",
            restore_path.display(),
            state.db_path.display()
        )));
    }

    // Logical restore succeeded, temporary restore file can be deleted.
    let _ = fs::remove_file(&restore_path).await;
    Ok(bak_path)
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

fn is_malformed_sqlite_error(err: &sqlx::Error) -> bool {
    let text = err.to_string().to_lowercase();
    text.contains("malformed") || text.contains("database disk image is malformed")
}

async fn validate_sqlite_image_bytes(state: &AppState, db_bytes: &[u8]) -> AppResult<()> {
    let backup_dir = state.backup_root_dir();
    fs::create_dir_all(&backup_dir).await?;
    let tmp_path = backup_dir.join(format!("validate-{}.db", uuid::Uuid::new_v4()));
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

    let check_result = match validation_result {
        Ok(v) => v,
        Err(e) => {
            return Err(AppError::BadRequest(format!(
                "备份数据库镜像不可用（完整性检查失败）: {}",
                e
            )))
        }
    };

    if check_result.to_lowercase() != "ok" {
        return Err(AppError::BadRequest(format!(
            "备份数据库镜像损坏，integrity_check={}",
            check_result
        )));
    }

    Ok(())
}

async fn restore_database_logically(state: &AppState, restore_path: &Path) -> AppResult<()> {
    let restore_path_str = restore_path
        .to_str()
        .ok_or_else(|| AppError::Anyhow(anyhow::anyhow!("restore path is not valid UTF-8")))?;

    let mut conn = state.pool.acquire().await?;

    // NOTE: In SQLite, changing foreign_keys inside a transaction has no effect.
    // We disable it at connection scope before starting the replacement transaction.
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

    let restore_result = async {
        // 直接使用 sqlite_master，因为 pragma_table_list 在某些 SQLite 版本中可能不可靠
        let table_rows = sqlx::query(
            "SELECT name
             FROM main.sqlite_master
             WHERE type='table'
               AND name NOT LIKE 'sqlite_%'
               AND sql IS NOT NULL
               AND sql NOT LIKE 'CREATE VIRTUAL TABLE%'",
        )
        .fetch_all(&mut *conn)
        .await?;

        sqlx::query("BEGIN IMMEDIATE").execute(&mut *conn).await?;

        for row in table_rows {
            let table_name: String = row.try_get("name")?;

            // 直接使用 sqlite_master 检查 restore_db 中的表
            let exists: i64 = sqlx::query_scalar(
                "SELECT COUNT(1)
                 FROM restore_db.sqlite_master
                 WHERE type='table'
                   AND name = ?
                   AND sql IS NOT NULL
                   AND sql NOT LIKE 'CREATE VIRTUAL TABLE%'",
            )
            .bind(&table_name)
            .fetch_one(&mut *conn)
            .await?;

            if exists == 0 {
                continue;
            }

            if !is_safe_sqlite_ident(&table_name) {
                tracing::warn!(
                    module = "backup",
                    event = "restore_skip_unsafe_table",
                    table = %table_name,
                    "skipping table with non-alphanumeric name during restore"
                );
                continue;
            }

            let qname = quote_sqlite_ident(&table_name);
            let delete_sql = format!("DELETE FROM {}", qname);
            let insert_sql = format!(
                "INSERT INTO {name} SELECT * FROM restore_db.{name}",
                name = qname
            );

            sqlx::query(&delete_sql).execute(&mut *conn).await?;
            sqlx::query(&insert_sql).execute(&mut *conn).await?;
        }

        sqlx::query("COMMIT").execute(&mut *conn).await?;
        Ok::<(), sqlx::Error>(())
    }
    .await;

    if restore_result.is_err() {
        let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
    }

    let _ = sqlx::query("DETACH DATABASE restore_db")
        .execute(&mut *conn)
        .await;
    let _ = sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&mut *conn)
        .await;

    if let Err(err) = restore_result {
        tracing::error!(error = ?err, "logical restore transaction failed");
        if is_malformed_sqlite_error(&err) {
            return Err(AppError::Anyhow(anyhow::anyhow!(
                "备份中的数据库镜像已损坏，无法自动恢复。请重新创建备份后再恢复。"
            )));
        }
        return Err(err.into());
    }
    tracing::info!("database restored via logical in-place restore");
    Ok(())
}

pub async fn create_backup(
    state: Arc<AppState>,
    provider: BackupProvider,
) -> AppResult<serde_json::Value> {
    let db_bytes = export_consistent_db_snapshot_strict(&state).await?;
    let manifest_hash = hash_bytes(&db_bytes);
    tracing::info!(
        module = "backup",
        event = "create_backup_snapshot_ready",
        db_size = db_bytes.len(),
        manifest_hash = %manifest_hash,
        provider = provider.as_str()
    );

    let cursor = Cursor::new(Vec::new());
    let mut writer = ZipWriter::new(cursor);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // Add database
    writer
        .start_file(BACKUP_DB_ENTRY, options)
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("写入备份数据库条目失败: {e}")))?;
    writer.write_all(&db_bytes)?;

    // Add media files
    let media_dir = state.upload_dir.join("media");
    if fs::try_exists(&media_dir).await? {
        let mut entries = fs::read_dir(&media_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_file() {
                let file_name = path
                    .file_name()
                    .ok_or_else(|| AppError::Anyhow(anyhow::anyhow!("无法获取文件名")))?;
                let file_name_str = file_name.to_string_lossy();
                let file_bytes = fs::read(&path).await?;

                writer
                    .start_file(format!("media/{}", file_name_str), options)
                    .map_err(|e| AppError::Anyhow(anyhow::anyhow!("写入媒体文件失败: {e}")))?;
                writer.write_all(&file_bytes)?;
            }
        }
    }

    let manifest = serde_json::json!({
        "version": BACKUP_MANIFEST_VERSION,
        "schema_version": CURRENT_SCHEMA_VERSION,
        "app_version": env!("CARGO_PKG_VERSION"),
        "provider": provider.as_str(),
        "created_at": Utc::now().to_rfc3339(),
        "manifest_hash": manifest_hash,
        "entries": [
            {
                "path": BACKUP_DB_ENTRY,
                "size": db_bytes.len()
            }
        ]
    });

    writer
        .start_file("manifest.json", options)
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("写入备份 manifest 失败: {e}")))?;
    writer.write_all(manifest.to_string().as_bytes())?;

    let archive = writer
        .finish()
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("完成备份压缩包失败: {e}")))?
        .into_inner();

    let size = archive.len() as i64;

    let backup_id =
        repository::create_backup(&state.pool, provider.as_str(), size, &manifest_hash).await?;
    let backend = local_backend(&state)?;
    let path = backend
        .save(&backup_id, BACKUP_ARCHIVE_NAME, &archive)
        .await?;

    if provider == BackupProvider::S3 {
        tracing::warn!(backup_id = %backup_id, "S3 backend requested but no runtime S3 config is wired yet, archive retained locally for compatibility");
    }

    Ok(serde_json::json!({
        "id": backup_id,
        "provider": provider.as_str(),
        "size": size,
        "manifest_hash": manifest_hash,
        "path": path,
    }))
}

pub async fn list_backups(state: Arc<AppState>) -> AppResult<Vec<BackupListResponse>> {
    let items = repository::list_backups(&state.pool).await?;
    Ok(items
        .into_iter()
        .map(|item| BackupListResponse {
            id: item.id,
            created_at: item.created_at,
            size: item.size,
            provider: item.provider,
            status: item.status,
            error_message: item.error_message,
        })
        .collect())
}

pub async fn delete_backup(state: Arc<AppState>, id: String) -> AppResult<serde_json::Value> {
    let backup = repository::get_backup(&state.pool, &id)
        .await?
        .ok_or(AppError::NotFound(format!("备份 '{}' 未找到", id)))?;

    let backend = local_backend(&state)?;
    backend.delete(&backup.id, BACKUP_ARCHIVE_NAME).await?;
    repository::delete_backup(&state.pool, &backup.id).await?;

    Ok(serde_json::json!({ "deleted": true, "id": backup.id }))
}

#[allow(dead_code)]
pub async fn restore_backup(
    state: Arc<AppState>,
    backup_id: String,
) -> AppResult<Vec<RestoreProgressResponse>> {
    let backup = repository::get_backup(&state.pool, &backup_id)
        .await?
        .ok_or(AppError::NotFound(format!("备份 '{}' 未找到", backup_id)))?;

    repository::update_backup_status(
        &state.pool,
        &backup.id,
        BackupStatus::Running.as_str(),
        None,
    )
    .await?;

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
        repository::update_backup_status(
            &state.pool,
            &backup.id,
            BackupStatus::Failed.as_str(),
            Some("manifest hash mismatch"),
        )
        .await?;
        return Err(AppError::BadRequest(
            "备份校验失败，manifest hash 不匹配".into(),
        ));
    }

    validate_sqlite_image_bytes(&state, &db_bytes).await?;
    tracing::info!(
        module = "backup",
        event = "restore_archive_validated",
        backup_id = %backup.id,
        db_size = db_bytes.len(),
        manifest_hash = %expected_hash
    );

    progress.push(RestoreProgressResponse {
        step: RestoreStep::Extract.as_str().to_string(),
        status: "completed".into(),
        message: "已解压备份包并验证 manifest".into(),
    });

    let bak_path = restore_database_file(&state, &db_bytes).await?;
    progress.push(RestoreProgressResponse {
        step: RestoreStep::Replace.as_str().to_string(),
        status: "completed".into(),
        message: format!("已替换数据库文件，旧文件保留在 {}", bak_path.display()),
    });

    progress.push(RestoreProgressResponse {
        step: RestoreStep::ClearCache.as_str().to_string(),
        status: "completed".into(),
        message: "当前版本无额外缓存，已跳过缓存清理".into(),
    });

    repository::update_backup_status(
        &state.pool,
        &backup.id,
        BackupStatus::Completed.as_str(),
        None,
    )
    .await?;

    Ok(progress)
}

pub async fn restore_backup_from_bytes(
    state: Arc<AppState>,
    bytes: Vec<u8>,
) -> AppResult<Vec<RestoreProgressResponse>> {
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
    tracing::info!(
        module = "backup",
        event = "restore_upload_validated",
        db_size = db_bytes.len(),
        manifest_hash = %expected_hash
    );

    progress.push(RestoreProgressResponse {
        step: RestoreStep::Extract.as_str().to_string(),
        status: "completed".into(),
        message: "已解压备份包并验证 manifest".into(),
    });

    // Extract media files
    let reader = Cursor::new(&bytes);
    let mut archive = ZipArchive::new(reader)
        .map_err(|e| AppError::BadRequest(format!("无法读取备份压缩包: {e}")))?;

    let media_dir = state.upload_dir.join("media");
    fs::create_dir_all(&media_dir).await?;

    // 解析 media_dir 的绝对路径，用于目录穿越检测（canonicalize 要求文件已存在，改用 absolute）
    let resolved_media_dir =
        std::path::absolute(&media_dir).unwrap_or_else(|_| media_dir.clone());

    const MAX_SINGLE_MEDIA_FILE_SIZE: u64 = 100 * 1024 * 1024; // 100MB 单文件上限，防 Zip Bomb

    let mut media_files: Vec<(PathBuf, Vec<u8>)> = Vec::new();
    for i in 0..archive.len() {
        let file = archive
            .by_index(i)
            .map_err(|e| AppError::BadRequest(format!("无法读取备份文件: {e}")))?;

        if file.name().starts_with("media/") && !file.is_dir() {
            let entry_name = file.name();
            let relative_name = entry_name
                .strip_prefix("media/")
                .unwrap_or(entry_name);

            // 第1层防护：拒绝包含危险路径组件的条目名
            if relative_name.contains("..")
                || relative_name.starts_with('/')
                || relative_name.starts_with('\\')
            {
                return Err(AppError::BadRequest(
                    "检测到非法的 ZIP 条目路径".into(),
                ));
            }

            // 第2层防护：拼接目标路径后验证绝对路径仍在 media_dir 内
            let dest_path = media_dir.join(relative_name);
            let resolved_dest =
                std::path::absolute(&dest_path).unwrap_or_else(|_| dest_path.clone());
            if !resolved_dest.starts_with(&resolved_media_dir) {
                return Err(AppError::BadRequest("目录穿越攻击拦截".into()));
            }

            // 第3层防护：限制单文件解压大小，防止 Zip Bomb
            let mut file_bytes = Vec::new();
            let mut limited = file.take(MAX_SINGLE_MEDIA_FILE_SIZE + 1);
            limited.read_to_end(&mut file_bytes)?;
            if file_bytes.len() as u64 > MAX_SINGLE_MEDIA_FILE_SIZE {
                return Err(AppError::BadRequest(format!(
                    "媒体文件超过单文件大小上限 {}MB",
                    MAX_SINGLE_MEDIA_FILE_SIZE / (1024 * 1024)
                )));
            }

            media_files.push((dest_path, file_bytes));
        }
    }

    for (dest_path, file_bytes) in media_files {
        // 确保父目录存在后再写入
        if let Some(parent) = dest_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&dest_path, file_bytes).await?;
    }

    let bak_path = restore_database_file(&state, &db_bytes).await?;
    progress.push(RestoreProgressResponse {
        step: RestoreStep::Replace.as_str().to_string(),
        status: "completed".into(),
        message: format!("已替换数据库文件，旧文件保留在 {}", bak_path.display()),
    });

    progress.push(RestoreProgressResponse {
        step: RestoreStep::ClearCache.as_str().to_string(),
        status: "completed".into(),
        message: "当前版本无额外缓存，已跳过缓存清理".into(),
    });

    Ok(progress)
}

pub async fn get_schedule(state: Arc<AppState>) -> AppResult<BackupScheduleResponse> {
    let schedule = repository::get_or_create_schedule(&state.pool).await?;
    Ok(BackupScheduleResponse {
        id: schedule.id,
        enabled: schedule.enabled,
        frequency: schedule.frequency,
        hour: schedule.hour as u32,
        minute: schedule.minute as u32,
        provider: schedule.provider,
        last_run_at: schedule.last_run_at,
        next_run_at: schedule.next_run_at,
        created_at: schedule.created_at,
        updated_at: schedule.updated_at,
    })
}

/// 将用户选择的站点时区本地时间转换为 UTC 时间，用于 cron 表达式计算
/// 例如用户选 Asia/Shanghai 的 02:00 → UTC 18:00（前一天）；默认 UTC
pub fn local_time_to_utc_for_cron(hour: u32, minute: u32, tz: Tz) -> (u32, u32) {
    let now = Utc::now();
    let local_result = tz.with_ymd_and_hms(now.year(), now.month(), now.day(), hour, minute, 0);
    let utc_time = match local_result {
        chrono::LocalResult::Single(dt) => dt.with_timezone(&Utc),
        _ => {
            // DST 跳变导致时间不存在或歧义，fallback 到今天的 UTC 同时间点
            Utc.with_ymd_and_hms(now.year(), now.month(), now.day(), hour, minute, 0)
                .unwrap()
        }
    };
    (utc_time.hour(), utc_time.minute())
}

fn calculate_next_run_at_from_cron_expression(cron_expression: &str) -> chrono::DateTime<Utc> {
    Schedule::from_str(cron_expression)
        .ok()
        .and_then(|schedule| schedule.upcoming(Utc).next())
        .unwrap_or_else(|| Utc::now() + chrono::Duration::hours(1))
}

pub async fn update_schedule(
    state: Arc<AppState>,
    request: BackupScheduleRequest,
) -> AppResult<BackupScheduleResponse> {
    let frequency = BackupScheduleFrequency::from_str(&request.frequency)
        .ok_or_else(|| AppError::BadRequest("frequency 仅支持 daily / weekly / monthly".into()))?;
    let provider = BackupProvider::from_str(&request.provider)
        .ok_or_else(|| AppError::BadRequest("provider 仅支持 local / s3".into()))?;

    if request.hour > 23 || request.minute > 59 {
        return Err(AppError::BadRequest("hour/minute 超出合法范围".into()));
    }

    repository::get_or_create_schedule(&state.pool).await?;

    repository::update_schedule(
        &state.pool,
        request.enabled,
        frequency.as_str(),
        request.hour as i32,
        request.minute as i32,
        provider.as_str(),
    )
    .await?;

    // Set next_run_at based on cron expression (do NOT set last_run_at here;
    // last_run_at is only updated when a backup actually executes).
    // 将用户选择的站点时区本地时间转为 UTC，确保 cron 在正确时刻触发
    if request.enabled {
        let tz_str = crate::modules::setting::repository::get_string(&state.pool, "site_timezone", "UTC")
            .await
            .unwrap_or_else(|_| "UTC".into());
        let tz: Tz = tz_str.parse().unwrap_or(chrono_tz::UTC);
        let (utc_hour, utc_minute) = local_time_to_utc_for_cron(request.hour, request.minute, tz);
        let cron = frequency.cron_expression(utc_hour, utc_minute);
        let next_run_at = calculate_next_run_at_from_cron_expression(&cron);
        repository::set_next_run_at(&state.pool, &next_run_at.to_rfc3339()).await?;
    }

    // Restart the cron scheduler so the new config takes effect immediately
    if let Err(err) = super::scheduler::restart_backup_scheduler(state.clone()).await {
        tracing::error!(error = ?err, "failed to restart backup scheduler after config update");
    }

    get_schedule(state).await
}
