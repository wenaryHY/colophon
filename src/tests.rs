/// InkForge 基础测试模块
///
/// 覆盖核心共享组件的基本行为。

#[cfg(test)]
mod shared_tests {
    use crate::shared::response::{ApiResponse, PaginatedResponse};

    #[test]
    fn api_response_success_has_code_zero() {
        let resp = ApiResponse::success("hello");
        assert_eq!(resp.code, 0);
        assert_eq!(resp.message, "ok");
        assert_eq!(resp.data, Some("hello"));
        assert!(!resp.request_id.is_empty());
    }

    #[test]
    fn api_response_error_has_no_data() {
        let resp = ApiResponse::<()>::error(40400, "not found");
        assert_eq!(resp.code, 40400);
        assert_eq!(resp.message, "not found");
        assert!(resp.data.is_none());
    }

    #[test]
    fn paginated_response_builds_correctly() {
        let resp = PaginatedResponse::new(vec![1, 2, 3], 1, 10, 100);
        assert_eq!(resp.items.len(), 3);
        assert_eq!(resp.pagination.page, 1);
        assert_eq!(resp.pagination.page_size, 10);
        assert_eq!(resp.pagination.total, 100);
    }
}

#[cfg(test)]
mod config_tests {
    use crate::bootstrap::config::AppConfig;

    #[test]
    fn default_config_loads_successfully() {
        // 确保默认配置可以加载（config/default.toml 存在）
        let config = AppConfig::load();
        assert!(
            config.is_ok(),
            "default config should load: {:?}",
            config.err()
        );
    }

    #[test]
    fn config_validate_allows_default_secret_in_development() {
        let config = AppConfig::load().unwrap();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn config_validate_blocks_default_secret_in_production() {
        let mut config = AppConfig::load().unwrap();
        config.runtime.mode = "production".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn resolve_path_handles_relative() {
        let result = AppConfig::resolve_path("uploads");
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.is_absolute());
        assert!(path.to_string_lossy().contains("uploads"));
    }
}

#[cfg(test)]
mod domain_tests {
    use crate::modules::backup::domain::{BackupProvider, BackupScheduleFrequency};

    #[test]
    fn backup_provider_roundtrips() {
        assert_eq!(
            BackupProvider::from_str("local"),
            Some(BackupProvider::Local)
        );
        assert_eq!(BackupProvider::from_str("s3"), Some(BackupProvider::S3));
        assert_eq!(BackupProvider::from_str("invalid"), None);
        assert_eq!(BackupProvider::Local.as_str(), "local");
        assert_eq!(BackupProvider::S3.as_str(), "s3");
    }

    #[test]
    fn backup_frequency_cron_expressions() {
        // cron crate v0.12 uses 7-field format: sec min hour dom month dow year
        let hourly = BackupScheduleFrequency::Hourly;
        assert_eq!(hourly.cron_expression(0, 30), "0 30 * * * * *");

        let daily = BackupScheduleFrequency::Daily;
        assert_eq!(daily.cron_expression(2, 30), "0 30 2 * * * *");

        let weekly = BackupScheduleFrequency::Weekly;
        assert_eq!(weekly.cron_expression(3, 0), "0 0 3 * * 0 *");

        let monthly = BackupScheduleFrequency::Monthly;
        assert_eq!(monthly.cron_expression(1, 15), "0 15 1 1 * * *");
    }
}

#[cfg(test)]
mod seo_tests {
    use crate::modules::seo::meta::{build_home_meta, generate_excerpt};

    #[test]
    fn generate_excerpt_strips_html_and_truncates() {
        let html = "<p>This is a <strong>bold</strong> test with <em>emphasis</em>.</p>";
        let excerpt = generate_excerpt(html, 20);
        assert!(!excerpt.contains('<'));
        assert!(excerpt.len() <= 25); // 20 chars + "…"
    }

    #[test]
    fn generate_excerpt_handles_short_text() {
        let text = "Short";
        let excerpt = generate_excerpt(text, 100);
        assert_eq!(excerpt, "Short");
        assert!(!excerpt.contains('…'));
    }

    #[test]
    fn build_home_meta_populates_all_fields() {
        let meta = build_home_meta("My Blog", "Description", "https://example.com", "blog", "");
        assert_eq!(meta.title, "My Blog");
        assert_eq!(meta.og_type, "website");
        assert_eq!(meta.canonical_url, "https://example.com");
        assert_eq!(meta.twitter_card, "summary"); // no image → summary
    }
}

#[cfg(test)]
mod theme_domain_tests {
    use crate::modules::theme::domain::ThemeManifest;

    #[test]
    fn theme_manifest_deserializes_minimal() {
        let toml_str = r#"
            name = "Test"
            slug = "test"
            version = "1.0.0"
            author = "Tester"
            description = "A test theme"
            min_inkforge_version = "0.3.0"
        "#;
        let manifest: ThemeManifest = toml::from_str(toml_str).unwrap();
        assert_eq!(manifest.slug, "test");
        assert!(manifest.config.is_empty());
        assert!(manifest.preview_image.is_none());
    }
}

// 新增集成测试模块
mod auth_tests;
mod post_tests;
mod setup_tests;
mod backup_tests;
mod theme_engine_tests;
mod theme_provider_tests;
mod plugin_manager_tests;
mod plugin_manifest_tests;
mod plugin_id_strategy_tests;
mod plugin_hook_tests;
mod plugin_settings_tests;
