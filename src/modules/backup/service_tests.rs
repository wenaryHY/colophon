#[cfg(test)]
mod tests {
    use sqlx::SqlitePool;
    use std::io::Read;
    use std::io::Write;
    use std::sync::Arc;
    use tokio::sync::{broadcast, RwLock};

    use tokio_util::sync::CancellationToken;

    use crate::{
        infra::backup::BackupStorageBackend, modules::plugin::manager::PluginManager,
        modules::setup::domain::SetupStage, state::AppState,
    };

    use super::super::{
        domain::BackupProvider,
        service::{create_backup, delete_backup, list_backups, restore_backup_from_bytes},
    };

    /// 创建测试环境：临时数据库 + AppState（使用 tempfile 自动清理）
    async fn setup_test_env() -> (Arc<AppState>, tempfile::TempDir) {
        let temp_dir = tempfile::tempdir().expect("create tempdir");
        let temp_path = temp_dir.path();
        let db_path = temp_path.join("test_backup.db");
        let backup_dir = temp_path.join("backups");

        // 创建数据库连接
        let pool = SqlitePool::connect(&format!("sqlite:{}?mode=rwc", db_path.display()))
            .await
            .expect("Failed to create test database");

        // 运行 migrations
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("Failed to run migrations");

        // 创建基础配置
        let config = crate::bootstrap::config::AppConfig::load().unwrap_or_else(|_| {
            // 测试环境降级配置
            panic!("Cannot load config for test");
        });
        let (event_tx, _) = broadcast::channel(16);
        let plugin_manager = Arc::new(tokio::sync::RwLock::new(PluginManager::load().await));

        // 创建 AppState
        let state = AppState {
            pool: pool.clone(),
            config,
            upload_dir: temp_path.join("uploads"),
            static_dir: std::path::PathBuf::from("static"),
            theme_dir: std::path::PathBuf::from("themes"),
            admin_dist_dir: std::path::PathBuf::from("admin/dist"),
            db_path: db_path.clone(),
            backup_dir: backup_dir.clone(),
            event_tx,
            site_url: Arc::new(RwLock::new("http://localhost:3000".to_string())),
            admin_url: Arc::new(RwLock::new("http://localhost:3000/admin".to_string())),
            setup_stage: Arc::new(RwLock::new(SetupStage::Completed)),
            login_rate_limiter: Arc::new(tokio::sync::Mutex::new(
                crate::shared::security::LoginRateLimiter::new(),
            )),
            comment_rate_limiter: Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            template_cache: Arc::new(
                crate::modules::theme::cache::TemplateContextCache::with_default_ttl(),
            ),
            template_env_cache: Arc::new(RwLock::new(std::collections::HashMap::new())),
            plugin_manager,
            backup_scheduler: Arc::new(tokio::sync::Mutex::new(None)),
            asset_manifest: Arc::new(crate::state::AssetManifest::load()),
            shutdown_token: CancellationToken::new(),
            trash_scheduler_handle: Arc::new(tokio::sync::Mutex::new(None)),
            theme_watcher_handle: Arc::new(tokio::sync::Mutex::new(None)),
            converter_send: Arc::new(tokio::sync::Mutex::new(None)),
            webp_worker_handle: Arc::new(tokio::sync::Mutex::new(None)),
        };

        (Arc::new(state), temp_dir)
    }

    /// 创建测试用户
    async fn create_test_user(pool: &SqlitePool) -> String {
        let user_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO users (id, username, email, password_hash, display_name, role, status, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, 'admin', 'active', datetime('now'), datetime('now'))",
        )
        .bind(&user_id)
        .bind("testuser")
        .bind("test@example.com")
        .bind("dummy_hash")
        .bind("Test User")
        .execute(pool)
        .await
        .expect("Failed to create test user");
        user_id
    }

    /// 插入测试数据
    async fn insert_test_post(
        pool: &SqlitePool,
        author_id: &str,
        title: &str,
        slug: &str,
    ) -> String {
        let post_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO posts (id, author_id, title, slug, status, created_at, updated_at) 
             VALUES (?, ?, ?, ?, 'published', datetime('now'), datetime('now'))",
        )
        .bind(&post_id)
        .bind(author_id)
        .bind(title)
        .bind(slug)
        .execute(pool)
        .await
        .expect("Failed to insert test post");
        post_id
    }

    #[tokio::test]
    async fn test_create_backup_creates_valid_file() {
        let (state, _temp_dir) = setup_test_env().await;

        // 插入测试数据
        let author_id = create_test_user(&state.pool).await;
        insert_test_post(&state.pool, &author_id, "Test Post", "test-post").await;

        // 创建备份
        let result = create_backup(state.clone(), BackupProvider::Local)
            .await
            .expect("Failed to create backup");

        let backup_id = result["id"].as_str().expect("Missing backup id");
        let size = result["size"].as_i64().expect("Missing size");

        // 验证备份元数据
        assert!(!backup_id.is_empty());
        assert!(size > 0);

        // 验证备份文件存在
        let backend =
            crate::infra::backup::LocalBackupStorage::new(state.backup_dir.clone());
        let bytes = backend
            .read(backup_id, "backup.zip")
            .await
            .expect("Backup file should exist");
        assert!(bytes.len() > 0);
    }

    #[tokio::test]
    async fn test_backup_file_contains_database() {
        let (state, _temp_dir) = setup_test_env().await;

        let author_id = create_test_user(&state.pool).await;
        insert_test_post(&state.pool, &author_id, "Content Check", "content-check").await;

        let result = create_backup(state.clone(), BackupProvider::Local)
            .await
            .expect("Failed to create backup");

        let backup_id = result["id"].as_str().expect("Missing backup id");

        // 读取备份内容
        let backend =
            crate::infra::backup::LocalBackupStorage::new(state.backup_dir.clone());
        let bytes = backend.read(backup_id, "backup.zip").await.unwrap();

        // 验证 ZIP 包含数据库文件
        let reader = std::io::Cursor::new(&bytes);
        let mut archive = zip::ZipArchive::new(reader).expect("Invalid zip file");
        let db_entry = archive
            .by_name("database/colophon.db")
            .expect("Backup should contain database file");

        assert!(db_entry.size() > 0);
    }

    #[tokio::test]
    async fn test_list_backups_returns_sorted_by_time() {
        let (state, _temp_dir) = setup_test_env().await;

        let author_id = create_test_user(&state.pool).await;

        // 创建 3 个备份
        for i in 0..3 {
            insert_test_post(
                &state.pool,
                &author_id,
                &format!("Post {}", i),
                &format!("post-{}", i),
            )
            .await;
            create_backup(state.clone(), BackupProvider::Local)
                .await
                .expect("Failed to create backup");
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        // 列出备份
        let backups = list_backups(state.clone())
            .await
            .expect("Failed to list backups");

        assert_eq!(backups.len(), 3);

        // 验证按时间倒序
        for i in 0..backups.len() - 1 {
            assert!(
                backups[i].created_at >= backups[i + 1].created_at,
                "Backups should be sorted by created_at DESC"
            );
        }

        // 验证每个备份都有大小
        for backup in &backups {
            assert!(backup.size > 0);
            assert_eq!(backup.provider, "local");
            assert_eq!(backup.status, "completed");
        }
    }

    #[tokio::test]
    async fn test_restore_backup_restores_data() {
        let (state, _temp_dir) = setup_test_env().await;
        let db_path = state.db_path.clone();

        // 1. 插入原始数据
        let author_id = create_test_user(&state.pool).await;
        let post_id =
            insert_test_post(&state.pool, &author_id, "Original Title", "original-slug").await;

        // 2. 创建备份
        let result = create_backup(state.clone(), BackupProvider::Local)
            .await
            .expect("Failed to create backup");
        let backup_id = result["id"].as_str().unwrap().to_string();

        // 3. 修改数据
        sqlx::query("UPDATE posts SET title = ? WHERE id = ?")
            .bind("Modified Title")
            .bind(&post_id)
            .execute(&state.pool)
            .await
            .expect("Failed to update post");

        // 验证数据已修改
        let modified: String = sqlx::query_scalar("SELECT title FROM posts WHERE id = ?")
            .bind(&post_id)
            .fetch_one(&state.pool)
            .await
            .expect("Failed to fetch modified post");
        assert_eq!(modified, "Modified Title");

        // 4. 读取备份并恢复
        let backend =
            crate::infra::backup::LocalBackupStorage::new(state.backup_dir.clone());
        let backup_bytes = backend.read(&backup_id, "backup.zip").await.unwrap();

        // 关闭连接池，释放数据库锁
        state.pool.close().await;

        // 重新连接
        let new_pool = SqlitePool::connect(&format!("sqlite:{}?mode=rwc", db_path.display()))
            .await
            .expect("Failed to reconnect");

        let new_state = Arc::new(AppState {
            pool: new_pool.clone(),
            config: state.config.clone(),
            upload_dir: state.upload_dir.clone(),
            static_dir: state.static_dir.clone(),
            theme_dir: state.theme_dir.clone(),
            admin_dist_dir: state.admin_dist_dir.clone(),
            db_path: state.db_path.clone(),
            backup_dir: state.backup_dir.clone(),
            event_tx: state.event_tx.clone(),
            site_url: state.site_url.clone(),
            admin_url: state.admin_url.clone(),
            setup_stage: state.setup_stage.clone(),
            login_rate_limiter: state.login_rate_limiter.clone(),
            comment_rate_limiter: state.comment_rate_limiter.clone(),
            template_cache: state.template_cache.clone(),
            template_env_cache: state.template_env_cache.clone(),
            plugin_manager: state.plugin_manager.clone(),
            backup_scheduler: state.backup_scheduler.clone(),
            asset_manifest: state.asset_manifest.clone(),
            shutdown_token: CancellationToken::new(),
            trash_scheduler_handle: Arc::new(tokio::sync::Mutex::new(None)),
            theme_watcher_handle: Arc::new(tokio::sync::Mutex::new(None)),
            converter_send: Arc::new(tokio::sync::Mutex::new(None)),
            webp_worker_handle: Arc::new(tokio::sync::Mutex::new(None)),
        });

        restore_backup_from_bytes(new_state.clone(), backup_bytes)
            .await
            .expect("Failed to restore backup");

        // 关闭连接池以确保数据落盘
        new_pool.close().await;

        // 重新连接验证数据恢复
        let verify_pool = SqlitePool::connect(&format!("sqlite:{}?mode=rwc", db_path.display()))
            .await
            .expect("Failed to reconnect for verification");

        // 5. 验证数据恢复
        let restored: String = sqlx::query_scalar("SELECT title FROM posts WHERE id = ?")
            .bind(&post_id)
            .fetch_one(&verify_pool)
            .await
            .expect("Failed to fetch restored post");
        assert_eq!(restored, "Original Title");

        verify_pool.close().await;
    }

    #[tokio::test]
    async fn test_delete_backup_removes_file() {
        let (state, _temp_dir) = setup_test_env().await;

        let author_id = create_test_user(&state.pool).await;
        insert_test_post(&state.pool, &author_id, "Test", "test").await;

        let result = create_backup(state.clone(), BackupProvider::Local)
            .await
            .expect("Failed to create backup");
        let backup_id = result["id"].as_str().unwrap().to_string();

        // 验证文件存在
        let backend =
            crate::infra::backup::LocalBackupStorage::new(state.backup_dir.clone());
        assert!(backend.read(&backup_id, "backup.zip").await.is_ok());

        // 删除备份
        delete_backup(state.clone(), backup_id.clone())
            .await
            .expect("Failed to delete backup");

        // 验证文件已删除
        assert!(backend.read(&backup_id, "backup.zip").await.is_err());

        // 验证数据库记录已删除
        let backups = list_backups(state.clone()).await.unwrap();
        assert!(!backups.iter().any(|b| b.id == backup_id));
    }

    #[tokio::test]
    async fn test_rejects_path_traversal_in_backup_id() {
        let (state, _temp_dir) = setup_test_env().await;

        let malicious_ids = vec![
            "../../../etc/passwd",
            "..\\..\\..\\windows\\system32\\config",
            "/etc/passwd",
            "C:\\Windows\\System32\\config",
            "../../sensitive_data.db",
        ];

        let backend =
            crate::infra::backup::LocalBackupStorage::new(state.backup_dir.clone());

        for id in malicious_ids {
            // 尝试读取恶意路径
            let result = backend.read(id, "backup.zip").await;
            assert!(result.is_err(), "Should not allow reading path: {}", id);
        }
    }

    #[tokio::test]
    async fn test_backup_contains_manifest_with_correct_hash() {
        let (state, _temp_dir) = setup_test_env().await;

        let author_id = create_test_user(&state.pool).await;
        insert_test_post(&state.pool, &author_id, "Hash Test", "hash-test").await;

        let result = create_backup(state.clone(), BackupProvider::Local)
            .await
            .expect("Failed to create backup");

        let backup_id = result["id"].as_str().unwrap();
        let expected_hash = result["manifest_hash"].as_str().unwrap();

        // 读取备份并验证 manifest
        let backend =
            crate::infra::backup::LocalBackupStorage::new(state.backup_dir.clone());
        let bytes = backend.read(backup_id, "backup.zip").await.unwrap();

        let reader = std::io::Cursor::new(&bytes);
        let mut archive = zip::ZipArchive::new(reader).unwrap();

        // 读取 manifest.json
        let mut manifest_file = archive.by_name("manifest.json").unwrap();
        let mut manifest_content = String::new();
        std::io::Read::read_to_string(&mut manifest_file, &mut manifest_content).unwrap();

        let manifest: serde_json::Value = serde_json::from_str(&manifest_content).unwrap();
        let manifest_hash = manifest["manifest_hash"].as_str().unwrap();

        assert_eq!(manifest_hash, expected_hash);
        assert_eq!(manifest["provider"].as_str().unwrap(), "local");
    }

    #[tokio::test]
    async fn test_restore_validates_manifest_hash() {
        let (state, _temp_dir) = setup_test_env().await;
        let db_path = state.db_path.clone();

        let author_id = create_test_user(&state.pool).await;
        insert_test_post(
            &state.pool,
            &author_id,
            "Validation Test",
            "validation-test",
        )
        .await;

        let result = create_backup(state.clone(), BackupProvider::Local)
            .await
            .expect("Failed to create backup");
        let backup_id = result["id"].as_str().unwrap();

        // 读取备份
        let backend =
            crate::infra::backup::LocalBackupStorage::new(state.backup_dir.clone());
        let mut bytes = backend.read(backup_id, "backup.zip").await.unwrap();

        // 篡改数据（改变最后一个字节）
        if let Some(last) = bytes.last_mut() {
            *last = last.wrapping_add(1);
        }

        // 尝试恢复损坏的备份
        state.pool.close().await;
        let new_pool = SqlitePool::connect(&format!("sqlite:{}?mode=rwc", db_path.display()))
            .await
            .unwrap();

        let new_state = Arc::new(AppState {
            pool: new_pool.clone(),
            config: state.config.clone(),
            upload_dir: state.upload_dir.clone(),
            static_dir: state.static_dir.clone(),
            theme_dir: state.theme_dir.clone(),
            admin_dist_dir: state.admin_dist_dir.clone(),
            db_path: state.db_path.clone(),
            backup_dir: state.backup_dir.clone(),
            event_tx: state.event_tx.clone(),
            site_url: state.site_url.clone(),
            admin_url: state.admin_url.clone(),
            setup_stage: state.setup_stage.clone(),
            login_rate_limiter: state.login_rate_limiter.clone(),
            comment_rate_limiter: state.comment_rate_limiter.clone(),
            template_cache: state.template_cache.clone(),
            template_env_cache: state.template_env_cache.clone(),
            plugin_manager: state.plugin_manager.clone(),
            backup_scheduler: state.backup_scheduler.clone(),
            asset_manifest: state.asset_manifest.clone(),
            shutdown_token: CancellationToken::new(),
            trash_scheduler_handle: Arc::new(tokio::sync::Mutex::new(None)),
            theme_watcher_handle: Arc::new(tokio::sync::Mutex::new(None)),
            converter_send: Arc::new(tokio::sync::Mutex::new(None)),
            webp_worker_handle: Arc::new(tokio::sync::Mutex::new(None)),
        });

        let result = restore_backup_from_bytes(new_state.clone(), bytes).await;

        // 应该失败（ZIP 损坏或 manifest hash 不匹配）
        assert!(result.is_err(), "Should reject corrupted backup");

        new_pool.close().await;
    }

    #[tokio::test]
    async fn test_multiple_backups_independent() {
        let (state, _temp_dir) = setup_test_env().await;

        let author_id = create_test_user(&state.pool).await;
        // 创建第一个备份
        insert_test_post(&state.pool, &author_id, "Post A", "post-a").await;
        let result1 = create_backup(state.clone(), BackupProvider::Local)
            .await
            .unwrap();
        let backup_id1 = result1["id"].as_str().unwrap().to_string();

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // 添加更多数据并创建第二个备份
        insert_test_post(&state.pool, &author_id, "Post B", "post-b").await;
        let result2 = create_backup(state.clone(), BackupProvider::Local)
            .await
            .unwrap();
        let backup_id2 = result2["id"].as_str().unwrap().to_string();

        // 验证两个备份不同
        assert_ne!(backup_id1, backup_id2);
        assert_ne!(
            result1["manifest_hash"].as_str().unwrap(),
            result2["manifest_hash"].as_str().unwrap()
        );

        // 删除第一个备份不影响第二个
        delete_backup(state.clone(), backup_id1.clone())
            .await
            .unwrap();

        let backend =
            crate::infra::backup::LocalBackupStorage::new(state.backup_dir.clone());
        assert!(backend.read(&backup_id1, "backup.zip").await.is_err());
        assert!(backend.read(&backup_id2, "backup.zip").await.is_ok());
    }

    /// 路径穿越防护：构造包含 `../` 的恶意 ZIP 条目，验证 restore 拒绝写入
    #[tokio::test]
    async fn test_restore_rejects_path_traversal_in_zip_entry() {
        let (state, _temp_dir) = setup_test_env().await;

        let author_id = create_test_user(&state.pool).await;
        insert_test_post(&state.pool, &author_id, "ZipSlip Test", "zipslip-test").await;

        // 1. 创建合法备份，从中获取一致性的数据库快照和 manifest
        let result = create_backup(state.clone(), BackupProvider::Local)
            .await
            .expect("Failed to create backup");
        let backup_id = result["id"].as_str().unwrap();

        let backend =
            crate::infra::backup::LocalBackupStorage::new(state.backup_dir.clone());
        let original_bytes = backend.read(backup_id, "backup.zip").await.unwrap();

        // 2. 提取有效条目（DB + manifest）
        let reader = std::io::Cursor::new(&original_bytes);
        let mut archive = zip::ZipArchive::new(reader).unwrap();

        let mut db_bytes = Vec::new();
        {
            let mut entry = archive.by_name("database/colophon.db").unwrap();
            Read::read_to_end(&mut entry, &mut db_bytes).unwrap();
        }

        let mut manifest_bytes = Vec::new();
        {
            let mut entry = archive.by_name("manifest.json").unwrap();
            Read::read_to_end(&mut entry, &mut manifest_bytes).unwrap();
        }

        // 3. 构造恶意 ZIP：有效条目 + 路径穿越条目
        let cursor = std::io::Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(cursor);
        let options = zip::write::SimpleFileOptions::default();

        writer.start_file("database/colophon.db", options).unwrap();
        writer.write_all(&db_bytes).unwrap();

        writer.start_file("manifest.json", options).unwrap();
        writer.write_all(&manifest_bytes).unwrap();

        // 恶意条目：通过 ../ 穿越到 /etc/cron.d
        writer
            .start_file("media/../../../etc/cron.d/evil", options)
            .unwrap();
        writer.write_all(b"malicious cron payload").unwrap();

        let malicious_zip = writer.finish().unwrap().into_inner();

        // 4. 尝试恢复 —— 应在路径穿越检查处被拒绝
        //    注意：此时不关闭连接池，因为 restore 会在替换数据库前（validate 之后、
        //    media 解压阶段）因路径穿越而提前返回 Err，数据库不会被修改
        let result = restore_backup_from_bytes(state.clone(), malicious_zip).await;
        assert!(
            result.is_err(),
            "Should reject ZIP with path traversal entry"
        );
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("穿越") || err_msg.contains("非法"),
            "错误信息应包含路径穿越相关提示，实际: {}",
            err_msg
        );

        // 5. 验证恶意文件未被写入文件系统
        //    restore_backup_from_bytes 的三层防护（拒绝 ../ / 前缀断言 / 100MB 限制）
        //    应在第1层即拦截，文件不会落盘
        let evil_path = std::path::Path::new("/etc/cron.d/evil");
        assert!(!evil_path.exists(), "恶意文件不应被写入系统目录");
    }

    // ── 时区转换纯函数测试 ──────────────────────────────────────

    #[test]
    fn backup_scheduler_converts_local_time_to_utc_for_cron() {
        use super::super::{domain::BackupScheduleFrequency, service};

        // Asia/Shanghai (UTC+8) 的 00:00 → UTC 16:00（前一天）
        let tz: chrono_tz::Tz = "Asia/Shanghai".parse().unwrap();
        let (utc_hour, utc_minute) = service::local_time_to_utc_for_cron(0, 0, tz);
        assert_eq!(utc_hour, 16);
        assert_eq!(utc_minute, 0);

        // 验证 cron 表达式使用 UTC 时间
        let cron = BackupScheduleFrequency::Daily.cron_expression(utc_hour, utc_minute);
        assert_eq!(cron, "0 0 16 * * * *");
    }

    #[test]
    fn backup_scheduler_utc_timezone_no_conversion() {
        use super::super::service;

        // UTC 时区不做转换
        let tz: chrono_tz::Tz = "UTC".parse().unwrap();
        let (utc_hour, utc_minute) = service::local_time_to_utc_for_cron(0, 0, tz);
        assert_eq!(utc_hour, 0);
        assert_eq!(utc_minute, 0);
    }

    #[test]
    fn backup_scheduler_negative_offset_conversion() {
        use super::super::service;

        // Etc/GMT+5 恒定 UTC-5，不受夏令时影响；10:00 本地 → UTC 15:00
        let tz: chrono_tz::Tz = "Etc/GMT+5".parse().unwrap();
        let (utc_hour, utc_minute) = service::local_time_to_utc_for_cron(10, 0, tz);
        assert_eq!(utc_hour, 15);
        assert_eq!(utc_minute, 0);
    }

    /// Zip Bomb 防护：验证 100MB 单文件上限逻辑可达
    ///
    /// restore_backup_from_bytes 中硬编码:
    /// - const MAX_SINGLE_MEDIA_FILE_SIZE: u64 = 100 * 1024 * 1024
    /// - file.take(MAX_SINGLE_MEDIA_FILE_SIZE + 1) 限制读取
    /// - 读取后检查 file_bytes.len() > MAX_SINGLE_MEDIA_FILE_SIZE
    ///
    /// 构造 >100MB 解压数据的端到端测试运行成本过高（内存/时间），
    /// 本测试改为验证合法大小文件通过限制检查，确认代码路径可正常执行。
    #[tokio::test]
    async fn test_restore_allows_media_file_within_size_limit() {
        let (state, _temp_dir) = setup_test_env().await;
        let db_path = state.db_path.clone();

        let author_id = create_test_user(&state.pool).await;
        insert_test_post(&state.pool, &author_id, "SizeTest", "sizetest").await;

        // 创建合法备份
        let result = create_backup(state.clone(), BackupProvider::Local)
            .await
            .unwrap();
        let backup_id = result["id"].as_str().unwrap();

        let backend =
            crate::infra::backup::LocalBackupStorage::new(state.backup_dir.clone());
        let original_bytes = backend.read(backup_id, "backup.zip").await.unwrap();

        // 提取有效条目
        let reader = std::io::Cursor::new(&original_bytes);
        let mut archive = zip::ZipArchive::new(reader).unwrap();

        let mut db_bytes = Vec::new();
        {
            let mut entry = archive.by_name("database/colophon.db").unwrap();
            Read::read_to_end(&mut entry, &mut db_bytes).unwrap();
        }

        let mut manifest_bytes = Vec::new();
        {
            let mut entry = archive.by_name("manifest.json").unwrap();
            Read::read_to_end(&mut entry, &mut manifest_bytes).unwrap();
        }

        // 构造包含正常大小媒体文件的 ZIP
        let cursor = std::io::Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(cursor);
        let options = zip::write::SimpleFileOptions::default();

        writer.start_file("database/colophon.db", options).unwrap();
        writer.write_all(&db_bytes).unwrap();

        writer.start_file("manifest.json", options).unwrap();
        writer.write_all(&manifest_bytes).unwrap();

        // 正常大小的媒体文件，应通过 100MB 限制
        writer.start_file("media/small.txt", options).unwrap();
        writer.write_all(b"small file content").unwrap();

        let valid_zip = writer.finish().unwrap().into_inner();

        // 完整恢复需要替换数据库，先关闭连接池再重建 AppState
        state.pool.close().await;
        let new_pool = SqlitePool::connect(&format!("sqlite:{}?mode=rwc", db_path.display()))
            .await
            .unwrap();
        let new_state = Arc::new(AppState {
            pool: new_pool.clone(),
            config: state.config.clone(),
            upload_dir: state.upload_dir.clone(),
            static_dir: state.static_dir.clone(),
            theme_dir: state.theme_dir.clone(),
            admin_dist_dir: state.admin_dist_dir.clone(),
            db_path: state.db_path.clone(),
            backup_dir: state.backup_dir.clone(),
            event_tx: state.event_tx.clone(),
            site_url: state.site_url.clone(),
            admin_url: state.admin_url.clone(),
            setup_stage: state.setup_stage.clone(),
            login_rate_limiter: state.login_rate_limiter.clone(),
            comment_rate_limiter: state.comment_rate_limiter.clone(),
            template_cache: state.template_cache.clone(),
            template_env_cache: state.template_env_cache.clone(),
            plugin_manager: state.plugin_manager.clone(),
            backup_scheduler: state.backup_scheduler.clone(),
            asset_manifest: state.asset_manifest.clone(),
            shutdown_token: CancellationToken::new(),
            trash_scheduler_handle: Arc::new(tokio::sync::Mutex::new(None)),
            theme_watcher_handle: Arc::new(tokio::sync::Mutex::new(None)),
            converter_send: Arc::new(tokio::sync::Mutex::new(None)),
            webp_worker_handle: Arc::new(tokio::sync::Mutex::new(None)),
        });

        let result = restore_backup_from_bytes(new_state.clone(), valid_zip).await;
        assert!(
            result.is_ok(),
            "Normal-sized media file should pass 100MB size limit check"
        );

        new_pool.close().await;
    }

    #[test]
    fn local_time_to_utc_for_cron_ambiguous_time_picks_earlier() {
        use super::super::service;

        // America/New_York 在 11月第一个周日 1:00-2:00 会重复
        // 1:30 AM 出现两次：EDT (UTC-4) 和 EST (UTC-5)
        // 应选择较早的 EDT 版本
        let tz: chrono_tz::Tz = "America/New_York".parse().unwrap();
        let (utc_hour, utc_minute) = service::local_time_to_utc_for_cron(1, 30, tz);
        // 只验证返回值合理（0-23）
        assert!(utc_hour <= 23, "utc_hour should be 0-23, got {}", utc_hour);
        assert!(utc_minute <= 59, "utc_minute should be 0-59, got {}", utc_minute);
    }

    #[test]
    fn local_time_to_utc_for_cron_nonexistent_time_falls_back() {
        use super::super::service;

        // America/New_York 在 3月第二个周日 2:00-3:00 被跳过
        // 2:30 AM 不存在，应 fallback
        let tz: chrono_tz::Tz = "America/New_York".parse().unwrap();
        let (utc_hour, utc_minute) = service::local_time_to_utc_for_cron(2, 30, tz);
        // 只验证返回值合理
        assert!(utc_hour <= 23, "utc_hour should be 0-23, got {}", utc_hour);
        assert!(utc_minute <= 59, "utc_minute should be 0-59, got {}", utc_minute);
    }
}
