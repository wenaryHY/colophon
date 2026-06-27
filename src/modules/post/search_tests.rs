#[cfg(test)]
mod fts5_search_tests {
    use crate::modules::post::{
        post_types::{ContentType, NewPostParams, PostStatus, Visibility},
        repository,
    };
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;

    async fn new_migrated_pool() -> sqlx::SqlitePool {
        let connect_options = SqliteConnectOptions::from_str("sqlite::memory:")
            .expect("parse sqlite url")
            .foreign_keys(false);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(connect_options)
            .await
            .expect("connect in-memory sqlite");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("run migrations");
        pool
    }

    async fn create_test_user(pool: &sqlx::SqlitePool, username: &str) -> String {
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

    async fn insert_published_post(
        pool: &sqlx::SqlitePool,
        author_id: &str,
        title: &str,
        content: &str,
    ) -> String {
        let post_id = repository::insert_post(
            pool,
            NewPostParams {
                author_id,
                title,
                slug: &slug::slugify(title),
                excerpt: Some(title),
                content_md: content,
                content_html: &format!("<p>{}</p>", content),
                cover_media_id: None,
                status: PostStatus::Published,
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
        .expect("insert post");
        post_id
    }

    #[tokio::test]
    async fn test_fts5_search_english_keyword() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "author").await;

        insert_published_post(&pool, &author_id, "Rust Programming", "Learn rust language").await;
        insert_published_post(&pool, &author_id, "Python Guide", "Python tutorial").await;
        insert_published_post(&pool, &author_id, "Go Concurrency", "Goroutines in Go").await;

        let results = repository::search_posts(&pool, "rust", None, None, 10, 0)
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust Programming");
    }

    #[tokio::test]
    async fn test_fts5_search_chinese_keyword() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "author").await;

        insert_published_post(
            &pool,
            &author_id,
            "Rust 性能优化",
            "深入理解 Rust 性能优化技巧",
        )
        .await;
        insert_published_post(&pool, &author_id, "Go 并发编程", "Go 语言并发模型").await;
        insert_published_post(&pool, &author_id, "性能测试工具", "常用性能测试工具介绍").await;

        let results = repository::search_posts(&pool, "性能", None, None, 10, 0)
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
        let titles: Vec<&str> = results.iter().map(|p| p.title.as_str()).collect();
        assert!(titles.contains(&"Rust 性能优化"));
        assert!(titles.contains(&"性能测试工具"));
    }

    #[tokio::test]
    async fn test_fts5_search_mixed_chinese_english() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "author").await;

        insert_published_post(&pool, &author_id, "Rust CMS 开发", "Build a CMS with Rust").await;
        insert_published_post(&pool, &author_id, "Python Web 框架", "Django and Flask").await;

        let results = repository::search_posts(&pool, "Rust", None, None, 10, 0)
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust CMS 开发");
    }

    #[tokio::test]
    async fn test_fts5_search_content_match() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "author").await;

        insert_published_post(
            &pool,
            &author_id,
            "Web Development",
            "SQLite FTS5 provides full-text search capabilities",
        )
        .await;
        insert_published_post(&pool, &author_id, "Database Guide", "PostgreSQL tutorial").await;

        let results = repository::search_posts(&pool, "SQLite", None, None, 10, 0)
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Web Development");
    }

    #[tokio::test]
    async fn test_fts5_search_no_results() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "author").await;

        insert_published_post(&pool, &author_id, "Rust Guide", "Learn Rust programming").await;

        let results = repository::search_posts(&pool, "nonexistent", None, None, 10, 0)
            .await
            .unwrap();

        assert_eq!(results.len(), 0);
    }

    #[tokio::test]
    async fn test_fts5_search_pagination() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "author").await;

        for i in 1..=5 {
            insert_published_post(
                &pool,
                &author_id,
                &format!("Rust Tutorial {}", i),
                "Learn Rust",
            )
            .await;
        }

        let page1 = repository::search_posts(&pool, "Rust", None, None, 2, 0)
            .await
            .unwrap();
        let page2 = repository::search_posts(&pool, "Rust", None, None, 2, 2)
            .await
            .unwrap();

        assert_eq!(page1.len(), 2);
        assert_eq!(page2.len(), 2);
        assert_ne!(page1[0].id, page2[0].id);
    }

    #[tokio::test]
    async fn test_fts5_count_search_results() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "author").await;

        for i in 1..=3 {
            insert_published_post(
                &pool,
                &author_id,
                &format!("Rust Guide {}", i),
                "Rust programming",
            )
            .await;
        }
        insert_published_post(&pool, &author_id, "Python Guide", "Python tutorial").await;

        let count = repository::count_search_posts(&pool, "Rust", None, None)
            .await
            .unwrap();

        assert_eq!(count, 3);
    }

    #[tokio::test]
    async fn test_fts5_only_searches_published_posts() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "author").await;

        // 创建已发布文章
        insert_published_post(&pool, &author_id, "Published Rust Post", "Rust content").await;

        // 创建草稿文章
        repository::insert_post(
            &pool,
            NewPostParams {
                author_id: &author_id,
                title: "Draft Rust Post",
                slug: "draft-rust-post",
                excerpt: None,
                content_md: "Rust draft content",
                content_html: "<p>Rust draft content</p>",
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
        .expect("insert draft post");

        let results = repository::search_posts(&pool, "Rust", None, None, 10, 0)
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Published Rust Post");
    }

    #[tokio::test]
    async fn test_fts5_excludes_deleted_posts() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "author").await;

        let post_id = insert_published_post(&pool, &author_id, "Rust Tutorial", "Learn Rust").await;

        // 验证能搜到
        let before = repository::search_posts(&pool, "Rust", None, None, 10, 0)
            .await
            .unwrap();
        assert_eq!(before.len(), 1);

        // 软删除
        repository::delete_post(&pool, &post_id).await.unwrap();

        // 验证搜不到
        let after = repository::search_posts(&pool, "Rust", None, None, 10, 0)
            .await
            .unwrap();
        assert_eq!(after.len(), 0);
    }

    #[tokio::test]
    async fn test_fts5_trigram_tokenizer_handles_partial_match() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "author").await;

        insert_published_post(
            &pool,
            &author_id,
            "SQLite Database",
            "SQLite is a powerful database",
        )
        .await;

        // trigram tokenizer 支持部分匹配
        let results = repository::search_posts(&pool, "SQL", None, None, 10, 0)
            .await
            .unwrap();

        // 如果 FTS5 trigram 生效，应该能搜到；如果降级到 LIKE，也能搜到
        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn test_fts5_update_post_updates_fts_index() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "author").await;

        let post_id =
            insert_published_post(&pool, &author_id, "Original Title", "Original content").await;

        // 验证能用旧标题搜到
        let before = repository::search_posts(&pool, "Original", None, None, 10, 0)
            .await
            .unwrap();
        assert_eq!(before.len(), 1);

        // 更新文章
        repository::update_post(
            &pool,
            crate::modules::post::post_types::UpdatePostParams {
                post_id: &post_id,
                title: "Updated Title",
                slug: "updated-title",
                excerpt: None,
                content_md: "Updated content",
                content_html: "<p>Updated content</p>",
                cover_media_id: None,
                status: PostStatus::Published,
                visibility: Visibility::Public,
                category_id: None,
                allow_comment: true,
                pinned: false,
                content_type: ContentType::Post,
                custom_html_path: None,
                page_render_mode: "editor",
                scheduled_at: None,
            },
            None,
        )
        .await
        .unwrap();

        // 验证旧标题搜不到
        let old_search = repository::search_posts(&pool, "Original", None, None, 10, 0)
            .await
            .unwrap();
        assert_eq!(old_search.len(), 0);

        // 验证新标题能搜到
        let new_search = repository::search_posts(&pool, "Updated", None, None, 10, 0)
            .await
            .unwrap();
        assert_eq!(new_search.len(), 1);
        assert_eq!(new_search[0].title, "Updated Title");
    }

    // ── Task 7: 搜索页测试 ──

    /// 创建 2 篇已发布文章（含不同内容），搜索关键词，验证返回匹配文章
    #[tokio::test]
    async fn test_search_keyword() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "author").await;

        insert_published_post(
            &pool,
            &author_id,
            "Learning Rust",
            "A comprehensive guide to Rust programming language",
        )
        .await;
        insert_published_post(
            &pool,
            &author_id,
            "Learning Python",
            "A comprehensive guide to Python programming",
        )
        .await;

        // 搜索 "Rust"，应只匹配第一篇
        let results = repository::search_posts(&pool, "Rust", None, None, 10, 0)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Learning Rust");

        // 搜索 "Python"，应只匹配第二篇
        let results = repository::search_posts(&pool, "Python", None, None, 10, 0)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Learning Python");
    }

    /// 空关键词搜索（keyword=""），验证不崩溃
    #[tokio::test]
    async fn test_search_empty() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "author").await;

        insert_published_post(&pool, &author_id, "Test Post", "Some content").await;

        // 空关键词可能导致 FTS5 语法错误，但不应该 panic
        let result = repository::search_posts(&pool, "", None, None, 10, 0).await;
        match result {
            Ok(results) => {
                let _ = results.len();
            }
            Err(_) => {
                // 返回错误是可以接受的，不 crash 即通过
            }
        }
    }

    /// 搜索不存在的关键词，验证返回空列表
    #[tokio::test]
    async fn test_search_no_results() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "author").await;

        insert_published_post(&pool, &author_id, "Rust Guide", "Learn Rust programming").await;

        let results = repository::search_posts(&pool, "nonexistent_keyword_xyz", None, None, 10, 0)
            .await
            .unwrap();
        assert_eq!(results.len(), 0);

        let count = repository::count_search_posts(&pool, "nonexistent_keyword_xyz", None, None)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }

    /// 创建 25 篇文章含相同关键词，搜索 page=2（page_size=10），验证分页正确
    #[tokio::test]
    async fn test_search_pagination() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "author").await;

        // 创建 25 篇含统一关键词的文章
        for i in 1..=25 {
            insert_published_post(
                &pool,
                &author_id,
                &format!("Rust Tutorial {}", i),
                "Learn Rust programming",
            )
            .await;
        }

        // page_size=10, page 1 (offset=0)
        let page1 = repository::search_posts(&pool, "Rust", None, None, 10, 0)
            .await
            .unwrap();
        assert_eq!(page1.len(), 10);

        // page_size=10, page 2 (offset=10)
        let page2 = repository::search_posts(&pool, "Rust", None, None, 10, 10)
            .await
            .unwrap();
        assert_eq!(page2.len(), 10);

        // page_size=10, page 3 (offset=20)
        let page3 = repository::search_posts(&pool, "Rust", None, None, 10, 20)
            .await
            .unwrap();
        assert_eq!(page3.len(), 5);

        // 验证各页不重叠
        let page1_ids: Vec<&str> = page1.iter().map(|p| p.id.as_str()).collect();
        let page2_ids: Vec<&str> = page2.iter().map(|p| p.id.as_str()).collect();
        let page3_ids: Vec<&str> = page3.iter().map(|p| p.id.as_str()).collect();
        for id in &page2_ids {
            assert!(!page1_ids.contains(id));
        }
        for id in &page3_ids {
            assert!(!page1_ids.contains(id));
            assert!(!page2_ids.contains(id));
        }

        // 验证总数
        let total = repository::count_search_posts(&pool, "Rust", None, None)
            .await
            .unwrap();
        assert_eq!(total, 25);
    }
}
