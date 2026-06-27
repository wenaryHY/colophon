//! `colophon export` 命令的集成测试。
//!
//! 测试策略：使用文件级 SQLite 数据库 + 临时目录，通过 `export::run()` 运行完整导出流程，
//! 然后读取生成的 JSON 文件验证数据完整性和正确性。

#[cfg(test)]
mod export_tests {
    use std::path::{Path, PathBuf};

    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use sqlx::SqlitePool;

    use crate::cli::export;
    use crate::modules::category::repository::insert_category;
    use crate::modules::post::post_types::{ContentType, NewPostParams, PostStatus, Visibility};
    use crate::modules::post::repository::{insert_post, replace_tags};
    use crate::modules::tag::repository::insert_tag;

    // ── 辅助函数 ────────────────────────────────────────────────────────

    /// 创建文件级 SQLite 数据库，运行所有迁移，返回连接池。
    async fn setup_file_db(db_path: &Path) -> SqlitePool {
        let _ = std::fs::remove_file(db_path);
        let options = SqliteConnectOptions::new()
            .filename(db_path)
            .create_if_missing(true)
            .foreign_keys(false);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("connect file db");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("run migrations");
        pool
    }

    /// 创建测试用户，返回 user_id。
    async fn create_test_user(pool: &SqlitePool, username: &str) -> String {
        let user_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO users (id, username, email, password_hash, role, display_name)
             VALUES (?, ?, ?, 'hash', 'admin', ?)",
        )
        .bind(&user_id)
        .bind(username)
        .bind(format!("{}@test.com", username))
        .bind(username)
        .execute(pool)
        .await
        .expect("create test user");
        user_id
    }

    /// 创建测试用临时目录，测试结束后调用方负责清理。
    fn create_temp_dir(prefix: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "colophon_export_test_{}_{}",
            prefix,
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    /// 清理临时目录。
    fn remove_temp_dir(dir: &Path) {
        let _ = std::fs::remove_dir_all(dir);
    }

    /// 读取 JSON 文件并解析为 `Vec<serde_json::Value>`。
    fn read_json_array(file_path: &Path) -> Vec<serde_json::Value> {
        let content = std::fs::read_to_string(file_path).expect("read JSON output file");
        serde_json::from_str(&content).expect("parse JSON array")
    }

    /// 辅助：创建一篇已发布文章。
    async fn create_published_post(
        pool: &SqlitePool,
        user_id: &str,
        title: &str,
        slug: &str,
        category_id: Option<&str>,
    ) -> String {
        insert_post(
            pool,
            NewPostParams {
                author_id: user_id,
                title,
                slug,
                excerpt: Some("excerpt"),
                content_md: "content",
                content_html: "<p>content</p>",
                cover_media_id: None,
                status: PostStatus::Published,
                visibility: Visibility::Public,
                category_id,
                allow_comment: true,
                pinned: false,
                content_type: ContentType::Post,
                custom_html_path: None,
                page_render_mode: "editor",
                scheduled_at: None,
            },
        )
        .await
        .expect("insert post")
    }

    /// 辅助：创建草稿文章。
    async fn create_draft_post(
        pool: &SqlitePool,
        user_id: &str,
        title: &str,
        slug: &str,
    ) -> String {
        insert_post(
            pool,
            NewPostParams {
                author_id: user_id,
                title,
                slug,
                excerpt: Some("draft excerpt"),
                content_md: "draft",
                content_html: "<p>draft</p>",
                cover_media_id: None,
                status: PostStatus::Draft,
                visibility: Visibility::Public,
                category_id: None,
                allow_comment: true,
                pinned: false,
                content_type: ContentType::Post,
                custom_html_path: None,
                page_render_mode: "editor",
                scheduled_at: None,
            },
        )
        .await
        .expect("insert draft post")
    }

    // ── 测试用例 ────────────────────────────────────────────────────────

    /// 创建 3 篇已发布文章 => export => posts.json 包含 3 条记录。
    #[tokio::test]
    async fn test_export_posts() {
        let temp_dir = create_temp_dir("posts");
        let db_path = temp_dir.join("colophon.db");
        let output_dir = temp_dir.join("output");
        let upload_dir = temp_dir.join("uploads");
        std::fs::create_dir_all(&upload_dir).unwrap();

        {
            let pool = setup_file_db(&db_path).await;
            let user_id = create_test_user(&pool, "author").await;
            for i in 1..=3 {
                let _ = create_published_post(
                    &pool,
                    &user_id,
                    &format!("Post {}", i),
                    &format!("post-{}", i),
                    None,
                )
                .await;
            }
        }

        export::run(db_path, output_dir.clone(), upload_dir)
            .await
            .expect("export run");

        let posts = read_json_array(&output_dir.join("posts.json"));
        assert_eq!(posts.len(), 3, "posts.json 应包含 3 条已发布文章记录");

        remove_temp_dir(&temp_dir);
    }

    /// 创建 2 个页面 => export => pages.json 包含 2 条记录。
    #[tokio::test]
    async fn test_export_pages() {
        let temp_dir = create_temp_dir("pages");
        let db_path = temp_dir.join("colophon.db");
        let output_dir = temp_dir.join("output");
        let upload_dir = temp_dir.join("uploads");
        std::fs::create_dir_all(&upload_dir).unwrap();

        {
            let pool = setup_file_db(&db_path).await;
            let user_id = create_test_user(&pool, "author").await;

            for i in 1..=2 {
                insert_post(
                    &pool,
                    NewPostParams {
                        author_id: &user_id,
                        title: &format!("Page {}", i),
                        slug: &format!("page-{}", i),
                        excerpt: Some("page excerpt"),
                        content_md: "page content",
                        content_html: "<p>page content</p>",
                        cover_media_id: None,
                        status: PostStatus::Published,
                        visibility: Visibility::Public,
                        category_id: None,
                        allow_comment: false,
                        pinned: false,
                        content_type: ContentType::Page,
                        custom_html_path: None,
                        page_render_mode: "editor",
                scheduled_at: None,
                    },
                )
                .await
                .expect("insert page");
            }
        }

        export::run(db_path, output_dir.clone(), upload_dir)
            .await
            .expect("export run");

        let pages = read_json_array(&output_dir.join("pages.json"));
        assert_eq!(pages.len(), 2, "pages.json 应包含 2 条页面记录");

        remove_temp_dir(&temp_dir);
    }

    /// 创建 1 篇已发布 + 1 篇草稿 => export => posts.json 只有 1 条（过滤草稿）。
    #[tokio::test]
    async fn test_export_filters_drafts() {
        let temp_dir = create_temp_dir("filters_drafts");
        let db_path = temp_dir.join("colophon.db");
        let output_dir = temp_dir.join("output");
        let upload_dir = temp_dir.join("uploads");
        std::fs::create_dir_all(&upload_dir).unwrap();

        {
            let pool = setup_file_db(&db_path).await;
            let user_id = create_test_user(&pool, "author").await;

            let _published =
                create_published_post(&pool, &user_id, "Published", "published", None).await;
            let _draft = create_draft_post(&pool, &user_id, "Draft", "draft").await;
        }

        export::run(db_path, output_dir.clone(), upload_dir)
            .await
            .expect("export run");

        let posts = read_json_array(&output_dir.join("posts.json"));
        assert_eq!(
            posts.len(),
            1,
            "posts.json 应只包含 1 条已发布文章，草稿被过滤"
        );

        remove_temp_dir(&temp_dir);
    }

    /// 创建文章关联 2 个标签 => export => tags.json 包含标签信息。
    #[tokio::test]
    async fn test_export_tags() {
        let temp_dir = create_temp_dir("tags");
        let db_path = temp_dir.join("colophon.db");
        let output_dir = temp_dir.join("output");
        let upload_dir = temp_dir.join("uploads");
        std::fs::create_dir_all(&upload_dir).unwrap();

        {
            let pool = setup_file_db(&db_path).await;
            let user_id = create_test_user(&pool, "author").await;

            let tag_rust_id = insert_tag(&pool, "Rust", "rust").await.expect("insert tag");
            let tag_go_id = insert_tag(&pool, "Go", "go").await.expect("insert tag");

            let post_id =
                create_published_post(&pool, &user_id, "Multi Tag Post", "multi-tag", None).await;

            replace_tags(&pool, &post_id, &[tag_rust_id, tag_go_id])
                .await
                .expect("replace tags");
        }

        export::run(db_path, output_dir.clone(), upload_dir)
            .await
            .expect("export run");

        let tags = read_json_array(&output_dir.join("tags.json"));
        assert_eq!(tags.len(), 2, "tags.json 应包含 2 个标签");

        let tag_names: Vec<&str> = tags.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(tag_names.contains(&"Rust"), "标签列表应包含 Rust");
        assert!(tag_names.contains(&"Go"), "标签列表应包含 Go");

        remove_temp_dir(&temp_dir);
    }

    /// 创建文章关联分类 => export => categories.json 包含分类。
    #[tokio::test]
    async fn test_export_categories() {
        let temp_dir = create_temp_dir("categories");
        let db_path = temp_dir.join("colophon.db");
        let output_dir = temp_dir.join("output");
        let upload_dir = temp_dir.join("uploads");
        std::fs::create_dir_all(&upload_dir).unwrap();

        {
            let pool = setup_file_db(&db_path).await;
            let user_id = create_test_user(&pool, "author").await;

            let cat_id = insert_category(
                &pool,
                "Technology",
                "technology",
                Some("tech desc"),
                None,
                0,
            )
            .await
            .expect("insert category");

            let _ = create_published_post(&pool, &user_id, "Tech Post", "tech-post", Some(&cat_id))
                .await;
        }

        export::run(db_path, output_dir.clone(), upload_dir)
            .await
            .expect("export run");

        let categories = read_json_array(&output_dir.join("categories.json"));
        assert_eq!(categories.len(), 1, "categories.json 应包含 1 个分类");
        assert_eq!(
            categories[0]["name"].as_str().unwrap(),
            "Technology",
            "分类名应为 Technology"
        );

        remove_temp_dir(&temp_dir);
    }

    /// 输出目录不存在时，export 自动创建。
    #[tokio::test]
    async fn test_export_creates_output_dir() {
        let temp_dir = create_temp_dir("creates_dir");
        let db_path = temp_dir.join("colophon.db");
        // 输出目录在 temp_dir 下，尚未存在
        let output_dir = temp_dir.join("should_be_created");
        let upload_dir = temp_dir.join("uploads");
        std::fs::create_dir_all(&upload_dir).unwrap();

        {
            let pool = setup_file_db(&db_path).await;
            let user_id = create_test_user(&pool, "author").await;
            let _ = create_published_post(&pool, &user_id, "Post", "post", None).await;
        }

        assert!(!output_dir.exists(), "输出目录在导出前不应存在");

        export::run(db_path, output_dir.clone(), upload_dir)
            .await
            .expect("export run");

        assert!(output_dir.exists(), "export 应自动创建输出目录");
        assert!(
            output_dir.join("posts.json").exists(),
            "输出目录应包含 posts.json"
        );

        remove_temp_dir(&temp_dir);
    }

    /// 导出 settings.json，包含 site_title 等默认设置。
    #[tokio::test]
    async fn test_export_settings() {
        let temp_dir = create_temp_dir("settings");
        let db_path = temp_dir.join("colophon.db");
        let output_dir = temp_dir.join("output");
        let upload_dir = temp_dir.join("uploads");
        std::fs::create_dir_all(&upload_dir).unwrap();

        {
            // 只创建数据库和运行迁移（迁移中已插入默认 settings）
            let _pool = setup_file_db(&db_path).await;
        }

        export::run(db_path, output_dir.clone(), upload_dir)
            .await
            .expect("export run");

        let settings = read_json_array(&output_dir.join("settings.json"));
        assert!(!settings.is_empty(), "settings.json 不应为空");

        // 验证 site_title 存在
        let site_title_entry = settings
            .iter()
            .find(|s| s["key"].as_str() == Some("site_title"))
            .expect("settings.json 应包含 site_title");
        assert_eq!(
            site_title_entry["value"].as_str().unwrap(),
            "Colophon",
            "site_title 默认值应为 Colophon"
        );

        remove_temp_dir(&temp_dir);
    }
}
