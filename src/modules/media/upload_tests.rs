/// Media 模块核心上传测试
/// 覆盖文件上传、大小限制、MIME 校验、路径安全、删除、查询等核心功能

#[cfg(test)]
mod upload_tests {
    use crate::{
        bootstrap::config::AppConfig,
        modules::media::{repository, service},
        shared::{auth::AuthUser, error::AppError, role::Role},
        state::AppState,
    };
    use sqlx::SqlitePool;
    use std::sync::Arc;
    use tokio::sync::broadcast;

    /// 创建测试用 AppState（内存数据库 + 临时上传目录）
    async fn setup_test_state() -> (Arc<AppState>, String) {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("Failed to create in-memory database");

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("Failed to run migrations");

        let upload_dir = format!(
            "target/test_uploads_{}",
            uuid::Uuid::new_v4().simple().to_string()
        );
        tokio::fs::create_dir_all(&upload_dir)
            .await
            .expect("Failed to create test upload dir");

        let mut config = AppConfig::load().unwrap_or_else(|_| {
            // 如果加载失败，提供默认测试配置
            panic!("Failed to load config for tests");
        });

        // 覆盖配置为测试值
        config.storage.upload_dir = upload_dir.clone();
        config.storage.max_upload_size_mb = 10;

        let (event_tx, _) = broadcast::channel(16);

        let state = AppState::new(
            config,
            pool,
            event_tx,
            "http://test.local".to_string(),
            "http://test.local/admin".to_string(),
            crate::modules::setup::domain::SetupStage::Completed,
            Arc::new(tokio::sync::RwLock::new(
                crate::modules::plugin::manager::PluginManager::load().await,
            )),
        )
        .expect("Failed to create AppState");

        (Arc::new(state), upload_dir)
    }

    fn cleanup_test_files(upload_dir: &str) {
        let _ = std::fs::remove_dir_all(upload_dir);
    }

    async fn create_test_auth_user(pool: &SqlitePool) -> AuthUser {
        let user_id = uuid::Uuid::new_v4().to_string();

        // 在数据库中创建测试用户，满足外键约束
        sqlx::query(
            "INSERT INTO users (id, username, email, password_hash, display_name, role, created_at) 
             VALUES (?, ?, ?, ?, ?, ?, datetime('now'))"
        )
        .bind(&user_id)
        .bind("test_user")
        .bind("test@example.com")
        .bind("$argon2id$v=19$m=19456,t=2,p=1$test_salt$test_hash") // dummy hash
        .bind("Test User") // 添加 display_name
        .bind("admin")
        .execute(pool)
        .await
        .expect("Failed to create test user");

        AuthUser {
            id: user_id,
            username: "test_user".to_string(),
            role: Role::Admin,
        }
    }

    #[tokio::test]
    async fn test_upload_jpeg_image() {
        let (state, upload_dir) = setup_test_state().await;
        let auth = create_test_auth_user(&state.pool).await;

        // JPEG 魔术字节
        let jpeg_bytes = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46];

        let result = service::upload_media_raw(
            state.clone(),
            &auth,
            "test_image.jpg".to_string(),
            Some("image/jpeg".to_string()),
            jpeg_bytes,
            None,
        )
        .await;

        assert!(result.is_ok(), "Upload should succeed");
        let media = result.unwrap();
        assert_eq!(media.mime_type, "image/jpeg");
        assert_eq!(media.kind, "image");
        assert_eq!(media.original_name, "test_image.jpg");
        assert!(media.size_bytes > 0);

        // 验证文件实际写入
        let file_path = state.upload_dir.join(&media.storage_path);
        assert!(
            tokio::fs::metadata(&file_path).await.is_ok(),
            "Uploaded file should exist on disk"
        );

        cleanup_test_files(&upload_dir);
    }

    #[tokio::test]
    async fn test_upload_png_image() {
        let (state, upload_dir) = setup_test_state().await;
        let auth = create_test_auth_user(&state.pool).await;

        // PNG 魔术字节
        let png_bytes = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

        let result = service::upload_media_raw(
            state.clone(),
            &auth,
            "test_image.png".to_string(),
            Some("image/png".to_string()),
            png_bytes,
            None,
        )
        .await;

        assert!(result.is_ok(), "PNG upload should succeed");
        let media = result.unwrap();
        assert_eq!(media.mime_type, "image/png");
        assert_eq!(media.kind, "image");

        cleanup_test_files(&upload_dir);
    }

    #[tokio::test]
    async fn test_upload_audio_mp3() {
        let (state, upload_dir) = setup_test_state().await;
        let auth = create_test_auth_user(&state.pool).await;

        // MP3 魔术字节（ID3v2 header）
        let mp3_bytes = vec![0x49, 0x44, 0x33, 0x03, 0x00, 0x00];

        let result = service::upload_media_raw(
            state.clone(),
            &auth,
            "test_audio.mp3".to_string(),
            Some("audio/mpeg".to_string()),
            mp3_bytes,
            None,
        )
        .await;

        assert!(result.is_ok(), "MP3 upload should succeed");
        let media = result.unwrap();
        assert_eq!(media.mime_type, "audio/mpeg");
        assert_eq!(media.kind, "audio");

        cleanup_test_files(&upload_dir);
    }

    #[tokio::test]
    async fn test_rejects_file_without_extension() {
        let (state, upload_dir) = setup_test_state().await;
        let auth = create_test_auth_user(&state.pool).await;

        let result = service::upload_media_raw(
            state.clone(),
            &auth,
            "no_extension".to_string(),
            Some("application/octet-stream".to_string()),
            vec![0u8; 100],
            None,
        )
        .await;

        assert!(result.is_err(), "Should reject file without extension");
        match result.unwrap_err() {
            AppError::BadRequest(msg) => {
                assert!(msg.contains("extension"));
            }
            _ => panic!("Expected BadRequest error"),
        }

        cleanup_test_files(&upload_dir);
    }

    #[tokio::test]
    async fn test_rejects_unsupported_file_type() {
        let (state, upload_dir) = setup_test_state().await;
        let auth = create_test_auth_user(&state.pool).await;

        let unsupported_types = vec![
            ("malicious.exe", "application/x-executable"),
            ("script.sh", "application/x-sh"),
            ("source.php", "text/x-php"),
            ("document.pdf", "application/pdf"), // PDF 不在当前白名单中
        ];

        for (filename, mime_type) in unsupported_types {
            let result = service::upload_media_raw(
                state.clone(),
                &auth,
                filename.to_string(),
                Some(mime_type.to_string()),
                vec![0u8; 100],
                None,
            )
            .await;

            assert!(
                result.is_err(),
                "Should reject unsupported type: {}",
                filename
            );
        }

        cleanup_test_files(&upload_dir);
    }

    #[tokio::test]
    async fn test_rejects_oversized_file() {
        let (state, upload_dir) = setup_test_state().await;
        let auth = create_test_auth_user(&state.pool).await;

        // 超过 max_upload_size_mb (10MB)
        let oversized = vec![0u8; 11 * 1024 * 1024];

        let result = service::upload_media_raw(
            state.clone(),
            &auth,
            "huge.jpg".to_string(),
            Some("image/jpeg".to_string()),
            oversized,
            None,
        )
        .await;

        assert!(result.is_err(), "Should reject oversized file");
        match result.unwrap_err() {
            AppError::BadRequest(msg) => {
                assert!(msg.contains("exceeds max size"));
            }
            _ => panic!("Expected BadRequest error"),
        }

        cleanup_test_files(&upload_dir);
    }

    #[tokio::test]
    async fn test_accepts_file_within_size_limit() {
        let (state, upload_dir) = setup_test_state().await;
        let auth = create_test_auth_user(&state.pool).await;

        // 正好 5MB
        let valid_size = vec![0xFF; 5 * 1024 * 1024];

        let result = service::upload_media_raw(
            state.clone(),
            &auth,
            "normal_size.jpg".to_string(),
            Some("image/jpeg".to_string()),
            valid_size,
            None,
        )
        .await;

        assert!(
            result.is_ok(),
            "Should accept file within size limit: {:?}",
            result.err()
        );

        cleanup_test_files(&upload_dir);
    }

    #[tokio::test]
    async fn test_mime_type_extension_mismatch_rejected() {
        let (state, upload_dir) = setup_test_state().await;
        let auth = create_test_auth_user(&state.pool).await;

        // 扩展名是 .jpg，但声明 MIME 为 audio/mpeg
        let result = service::upload_media_raw(
            state.clone(),
            &auth,
            "fake.jpg".to_string(),
            Some("audio/mpeg".to_string()),
            vec![0xFF; 100],
            None,
        )
        .await;

        assert!(
            result.is_err(),
            "Should reject MIME type / extension mismatch"
        );
        match result.unwrap_err() {
            AppError::BadRequest(msg) => {
                assert!(msg.contains("does not match"));
            }
            _ => panic!("Expected BadRequest error"),
        }

        cleanup_test_files(&upload_dir);
    }

    #[tokio::test]
    async fn test_generates_unique_storage_paths() {
        let (state, upload_dir) = setup_test_state().await;
        let auth = create_test_auth_user(&state.pool).await;

        let bytes = vec![0xFF; 100];

        let media1 = service::upload_media_raw(
            state.clone(),
            &auth,
            "duplicate.jpg".to_string(),
            Some("image/jpeg".to_string()),
            bytes.clone(),
            None,
        )
        .await
        .expect("First upload should succeed");

        let media2 = service::upload_media_raw(
            state.clone(),
            &auth,
            "duplicate.jpg".to_string(),
            Some("image/jpeg".to_string()),
            bytes.clone(),
            None,
        )
        .await
        .expect("Second upload should succeed");

        // 两次上传应生成不同的存储路径
        assert_ne!(
            media1.storage_path, media2.storage_path,
            "Storage paths should be unique"
        );
        assert_ne!(
            media1.stored_name, media2.stored_name,
            "Stored names should be unique"
        );

        cleanup_test_files(&upload_dir);
    }

    #[tokio::test]
    async fn test_soft_delete_media() {
        let (state, upload_dir) = setup_test_state().await;
        let auth = create_test_auth_user(&state.pool).await;

        let media = service::upload_media_raw(
            state.clone(),
            &auth,
            "to_delete.jpg".to_string(),
            Some("image/jpeg".to_string()),
            vec![0xFF; 100],
            None,
        )
        .await
        .expect("Upload should succeed");

        let file_path = state.upload_dir.join(&media.storage_path);

        // 删除媒体
        let delete_result = service::delete_media(state.clone(), &media.id).await;
        assert!(delete_result.is_ok(), "Delete should succeed");

        // 软删除后，查询应返回 NotFound
        let get_result = repository::get_media(&state.pool, &media.id).await;
        assert!(
            get_result.is_ok(),
            "Repository query should not error: {:?}",
            get_result.err()
        );
        assert!(
            get_result.unwrap().is_none(),
            "Soft-deleted media should not be returned"
        );

        // 文件应被物理删除（当前实现是物理删除）
        assert!(
            tokio::fs::metadata(&file_path).await.is_err(),
            "File should be removed from disk"
        );

        cleanup_test_files(&upload_dir);
    }

    #[tokio::test]
    async fn test_list_media_by_kind() {
        let (state, upload_dir) = setup_test_state().await;
        let auth = create_test_auth_user(&state.pool).await;

        // 上传图片
        service::upload_media_raw(
            state.clone(),
            &auth,
            "image1.jpg".to_string(),
            Some("image/jpeg".to_string()),
            vec![0xFF; 100],
            None,
        )
        .await
        .expect("Image upload should succeed");

        // 上传音频
        service::upload_media_raw(
            state.clone(),
            &auth,
            "audio1.mp3".to_string(),
            Some("audio/mpeg".to_string()),
            vec![0x49; 100],
            None,
        )
        .await
        .expect("Audio upload should succeed");

        // 按类型查询
        let images = repository::list_media(&state.pool, Some("image"), None, None, 10, 0)
            .await
            .expect("List images should succeed");
        let audios = repository::list_media(&state.pool, Some("audio"), None, None, 10, 0)
            .await
            .expect("List audios should succeed");

        assert_eq!(images.len(), 1, "Should have 1 image");
        assert_eq!(audios.len(), 1, "Should have 1 audio");
        assert!(images.iter().all(|m| m.kind == "image"));
        assert!(audios.iter().all(|m| m.kind == "audio"));

        cleanup_test_files(&upload_dir);
    }

    #[tokio::test]
    async fn test_list_media_pagination() {
        let (state, upload_dir) = setup_test_state().await;
        let auth = create_test_auth_user(&state.pool).await;

        // 上传 5 个文件
        for i in 0..5 {
            service::upload_media_raw(
                state.clone(),
                &auth,
                format!("image_{}.jpg", i),
                Some("image/jpeg".to_string()),
                vec![0xFF; 100],
                None,
            )
            .await
            .expect("Upload should succeed");
        }

        // 第一页（limit=2, offset=0）
        let page1 = repository::list_media(&state.pool, None, None, None, 2, 0)
            .await
            .expect("List page 1 should succeed");

        // 第二页（limit=2, offset=2）
        let page2 = repository::list_media(&state.pool, None, None, None, 2, 2)
            .await
            .expect("List page 2 should succeed");

        assert_eq!(page1.len(), 2, "Page 1 should have 2 items");
        assert_eq!(page2.len(), 2, "Page 2 should have 2 items");

        // 验证不重复
        let id1 = &page1[0].id;
        let id2 = &page2[0].id;
        assert_ne!(id1, id2, "Pages should contain different items");

        cleanup_test_files(&upload_dir);
    }

    #[tokio::test]
    async fn test_count_media() {
        let (state, upload_dir) = setup_test_state().await;
        let auth = create_test_auth_user(&state.pool).await;

        // 上传 3 个图片
        for i in 0..3 {
            service::upload_media_raw(
                state.clone(),
                &auth,
                format!("image_{}.jpg", i),
                Some("image/jpeg".to_string()),
                vec![0xFF; 100],
                None,
            )
            .await
            .expect("Upload should succeed");
        }

        let total = repository::count_media(&state.pool, None, None, None)
            .await
            .expect("Count should succeed");
        let image_count = repository::count_media(&state.pool, Some("image"), None, None)
            .await
            .expect("Count images should succeed");

        assert_eq!(total, 3, "Should have 3 total media");
        assert_eq!(image_count, 3, "Should have 3 images");

        cleanup_test_files(&upload_dir);
    }

    #[tokio::test]
    async fn test_search_media_by_keyword() {
        let (state, upload_dir) = setup_test_state().await;
        let auth = create_test_auth_user(&state.pool).await;

        service::upload_media_raw(
            state.clone(),
            &auth,
            "vacation_photo.jpg".to_string(),
            Some("image/jpeg".to_string()),
            vec![0xFF; 100],
            None,
        )
        .await
        .expect("Upload should succeed");

        service::upload_media_raw(
            state.clone(),
            &auth,
            "work_document.png".to_string(),
            Some("image/png".to_string()),
            vec![0x89; 100],
            None,
        )
        .await
        .expect("Upload should succeed");

        // 搜索 "vacation"
        let results = repository::list_media(&state.pool, None, Some("vacation"), None, 10, 0)
            .await
            .expect("Search should succeed");

        assert_eq!(results.len(), 1, "Should find 1 result");
        assert!(results[0].original_name.contains("vacation"));

        cleanup_test_files(&upload_dir);
    }

    #[tokio::test]
    async fn test_rename_media() {
        let (state, upload_dir) = setup_test_state().await;
        let auth = create_test_auth_user(&state.pool).await;

        let media = service::upload_media_raw(
            state.clone(),
            &auth,
            "old_name.jpg".to_string(),
            Some("image/jpeg".to_string()),
            vec![0xFF; 100],
            None,
        )
        .await
        .expect("Upload should succeed");

        // 重命名
        let rename_result = service::rename_media(state.clone(), &media.id, "new_name.jpg").await;
        assert!(rename_result.is_ok(), "Rename should succeed");

        // 验证新名称
        let updated = repository::get_media(&state.pool, &media.id)
            .await
            .expect("Query should succeed")
            .expect("Media should exist");

        assert_eq!(updated.original_name, "new_name.jpg");

        cleanup_test_files(&upload_dir);
    }

    #[tokio::test]
    async fn test_update_media_category() {
        let (state, upload_dir) = setup_test_state().await;
        let auth = create_test_auth_user(&state.pool).await;

        let media = service::upload_media_raw(
            state.clone(),
            &auth,
            "test.jpg".to_string(),
            Some("image/jpeg".to_string()),
            vec![0xFF; 100],
            None,
        )
        .await
        .expect("Upload should succeed");

        // 更新分类
        let update_result =
            service::update_category(state.clone(), &media.id, Some("portfolio")).await;
        assert!(update_result.is_ok(), "Update category should succeed");

        // 验证分类
        let updated = repository::get_media(&state.pool, &media.id)
            .await
            .expect("Query should succeed")
            .expect("Media should exist");

        assert_eq!(updated.category, Some("portfolio".to_string()));

        cleanup_test_files(&upload_dir);
    }

    #[tokio::test]
    async fn test_file_metadata_saved_correctly() {
        let (state, upload_dir) = setup_test_state().await;
        let auth = create_test_auth_user(&state.pool).await;

        let original_name = "test_metadata.jpg";
        let content_type = "image/jpeg";
        let file_bytes = vec![0xFF; 1024]; // 1KB

        let media = service::upload_media_raw(
            state.clone(),
            &auth,
            original_name.to_string(),
            Some(content_type.to_string()),
            file_bytes.clone(),
            None,
        )
        .await
        .expect("Upload should succeed");

        assert_eq!(media.original_name, original_name);
        assert_eq!(media.mime_type, content_type);
        assert_eq!(media.size_bytes, file_bytes.len() as i64);
        assert_eq!(media.uploader_id, auth.id);
        assert!(!media.id.is_empty());
        assert!(!media.stored_name.is_empty());
        assert!(!media.storage_path.is_empty());
        assert!(!media.public_url.is_empty());

        cleanup_test_files(&upload_dir);
    }

    #[tokio::test]
    async fn test_storage_path_uses_media_subdirectory() {
        let (state, upload_dir) = setup_test_state().await;
        let auth = create_test_auth_user(&state.pool).await;

        let media = service::upload_media_raw(
            state.clone(),
            &auth,
            "test.jpg".to_string(),
            Some("image/jpeg".to_string()),
            vec![0xFF; 100],
            None,
        )
        .await
        .expect("Upload should succeed");

        // storage_path 应该以 "media/" 开头
        assert!(
            media.storage_path.starts_with("media/") || media.storage_path.starts_with("media\\"),
            "Storage path should start with 'media/': {}",
            media.storage_path
        );

        cleanup_test_files(&upload_dir);
    }

    #[tokio::test]
    async fn test_public_url_format() {
        let (state, upload_dir) = setup_test_state().await;
        let auth = create_test_auth_user(&state.pool).await;

        let media = service::upload_media_raw(
            state.clone(),
            &auth,
            "test.jpg".to_string(),
            Some("image/jpeg".to_string()),
            vec![0xFF; 100],
            None,
        )
        .await
        .expect("Upload should succeed");

        // public_url 应该以 "/uploads/" 开头
        assert!(
            media.public_url.starts_with("/uploads/"),
            "Public URL should start with '/uploads/': {}",
            media.public_url
        );

        cleanup_test_files(&upload_dir);
    }

    // ==================== 安全测试（P0 优先级）====================

    #[tokio::test]
    async fn test_rejects_path_traversal_attack() {
        let (state, upload_dir) = setup_test_state().await;
        let auth = create_test_auth_user(&state.pool).await;

        let malicious_names = vec![
            "../../../etc/passwd",
            "..\\..\\..\\Windows\\System32\\config\\SAM",
            "innocent/../../../evil.sh",
            "foo/../../bar.jpg",
            "/etc/shadow",                    // 绝对路径
            "C:\\Windows\\System32\\evil.exe", // Windows 绝对路径
            "..\\evil.jpg",
            "./../../etc/hosts",
        ];

        for filename in malicious_names {
            let result = service::upload_media_raw(
                state.clone(),
                &auth,
                filename.to_string(),
                Some("image/jpeg".to_string()),
                vec![0xFF, 0xD8, 0xFF, 0xE0], // JPEG 魔术字节
                None,
            )
            .await;

            // 主要验证：应该拒绝
            if result.is_err() {
                continue; // ✅ 正确拒绝
            }

            // 如果侥幸通过，验证最终路径没有逃出上传目录
            let media = result.unwrap();
            let final_path = state.upload_dir.join(&media.storage_path);

            // 规范化路径（解析所有 .. 和符号链接）
            let canonical = std::fs::canonicalize(&final_path).unwrap_or_else(|_| final_path.clone());
            let upload_canonical = std::fs::canonicalize(&state.upload_dir).unwrap();

            assert!(
                canonical.starts_with(&upload_canonical),
                "❌ SECURITY VIOLATION: File path escaped upload directory!\n  \
                 Malicious filename: {}\n  \
                 Final path: {:?}\n  \
                 Upload dir: {:?}",
                filename,
                canonical,
                upload_canonical
            );
        }

        cleanup_test_files(&upload_dir);
    }

    #[tokio::test]
    async fn test_rejects_html_javascript_mime_disguise() {
        let (state, upload_dir) = setup_test_state().await;
        let auth = create_test_auth_user(&state.pool).await;

        let xss_vectors = vec![
            ("evil.jpg", "text/html"),
            ("script.png", "application/javascript"),
            ("payload.gif", "text/x-shellscript"),
            ("xss.webp", "text/javascript"),
        ];

        for (filename, mime_type) in xss_vectors {
            let result = service::upload_media_raw(
                state.clone(),
                &auth,
                filename.to_string(),
                Some(mime_type.to_string()),
                vec![0xFF; 100],
                None,
            )
            .await;

            assert!(
                result.is_err(),
                "Should reject XSS vector: {} with MIME {}",
                filename,
                mime_type
            );

            if let Err(AppError::BadRequest(msg)) = result {
                assert!(
                    msg.contains("not allowed") || msg.contains("unsupported"),
                    "Error message should indicate unsupported type: {}",
                    msg
                );
            }
        }

        cleanup_test_files(&upload_dir);
    }

    #[tokio::test]
    async fn test_case_insensitive_extension_handling() {
        let (state, upload_dir) = setup_test_state().await;
        let auth = create_test_auth_user(&state.pool).await;

        let variants = vec!["test.jpg", "test.JPG", "test.Jpg", "test.jPeG"];

        for filename in variants {
            let result = service::upload_media_raw(
                state.clone(),
                &auth,
                filename.to_string(),
                Some("image/jpeg".to_string()),
                vec![0xFF, 0xD8, 0xFF, 0xE0], // JPEG 魔术字节
                None,
            )
            .await;

            assert!(result.is_ok(), "Should accept case variation: {}", filename);

            let media = result.unwrap();
            assert_eq!(media.mime_type, "image/jpeg");
        }

        cleanup_test_files(&upload_dir);
    }

    #[tokio::test]
    async fn test_rejects_null_byte_injection() {
        let (state, upload_dir) = setup_test_state().await;
        let auth = create_test_auth_user(&state.pool).await;

        let null_byte_attacks = vec![
            "evil.jpg\0.exe",
            "safe.png\x00.sh",
            "payload.gif\0.php",
        ];

        for filename in null_byte_attacks {
            let result = service::upload_media_raw(
                state.clone(),
                &auth,
                filename.to_string(),
                Some("image/jpeg".to_string()),
                vec![0xFF, 0xD8, 0xFF, 0xE0],
                None,
            )
            .await;

            assert!(
                result.is_err(),
                "Should reject null byte injection: {:?}",
                filename.as_bytes()
            );
        }

        cleanup_test_files(&upload_dir);
    }
}
