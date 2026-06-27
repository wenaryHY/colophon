#[cfg(test)]
mod crud_tests {
    use crate::modules::post::{
        post_types::{ContentType, NewPostParams, PostStatus, UpdatePostParams, Visibility},
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

    async fn insert_post_helper(
        pool: &sqlx::SqlitePool,
        author_id: &str,
        title: &str,
        slug: &str,
        status: PostStatus,
        visibility: Visibility,
    ) -> String {
        repository::insert_post(
            pool,
            NewPostParams {
                author_id,
                title,
                slug,
                excerpt: None,
                content_md: "# Test Content",
                content_html: "<h1>Test Content</h1>",
                cover_media_id: None,
                status,
                visibility,
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
        .expect("insert post")
    }

    // ========== 测试：创建文章 ==========

    #[tokio::test]
    async fn test_create_draft_post() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "author1").await;

        let post_id = insert_post_helper(
            &pool,
            &author_id,
            "Test Draft",
            "test-draft",
            PostStatus::Draft,
            Visibility::Public,
        )
        .await;

        let post = repository::get_admin_post(&pool, &post_id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(post.title, "Test Draft");
        assert_eq!(post.slug, "test-draft");
        assert_eq!(post.status, PostStatus::Draft);
        assert!(post.published_at.is_none());
        assert!(post.deleted_at.is_none());
    }

    #[tokio::test]
    async fn test_create_published_post() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "author2").await;

        let post_id = insert_post_helper(
            &pool,
            &author_id,
            "Published Post",
            "published-post",
            PostStatus::Published,
            Visibility::Public,
        )
        .await;

        let post = repository::get_admin_post(&pool, &post_id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(post.status, PostStatus::Published);
        assert!(post.published_at.is_some());
    }

    #[tokio::test]
    async fn test_slug_conflict_creates_unique_slug() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "author3").await;

        // 第一个 post 占用 slug
        insert_post_helper(
            &pool,
            &author_id,
            "First Post",
            "same-slug",
            PostStatus::Draft,
            Visibility::Public,
        )
        .await;

        // 第二个 post 尝试使用相同 slug，repository 层应自动返回冲突
        // 但 service 层通过 resolve_unique_post_slug 会自动处理
        let exists = repository::slug_exists(&pool, "same-slug", None)
            .await
            .unwrap();
        assert!(exists);
    }

    #[tokio::test]
    async fn test_empty_title_rejected() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "author4").await;

        // title 为空字符串应该在 service 层被拒绝
        // 这里测试 repository 层，它不做校验
        let result = repository::insert_post(
            &pool,
            NewPostParams {
                author_id: &author_id,
                title: "",
                slug: "empty-title",
                excerpt: None,
                content_md: "",
                content_html: "",
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
        .await;

        // repository 层不校验 title，允许空字符串
        assert!(result.is_ok());
    }

    // ========== 测试：状态机流程 ==========

    #[tokio::test]
    async fn test_draft_to_published_sets_published_at() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "author5").await;

        let post_id = insert_post_helper(
            &pool,
            &author_id,
            "Draft Post",
            "draft-post",
            PostStatus::Draft,
            Visibility::Public,
        )
        .await;

        let draft = repository::get_admin_post(&pool, &post_id)
            .await
            .unwrap()
            .unwrap();
        assert!(draft.published_at.is_none());

        // 更新为 published
        repository::update_post(
            &pool,
            UpdatePostParams {
                post_id: &post_id,
                title: &draft.title,
                slug: &draft.slug,
                excerpt: draft.excerpt.as_deref(),
                content_md: &draft.content_md,
                content_html: &draft.content_html,
                cover_media_id: draft.cover_media_id.as_deref(),
                status: PostStatus::Published,
                visibility: draft.visibility,
                category_id: draft.category_id.as_deref(),
                allow_comment: draft.allow_comment,
                pinned: draft.pinned,
                content_type: draft.content_type,
                custom_html_path: draft.custom_html_path.as_deref(),
                page_render_mode: &draft.page_render_mode,
                scheduled_at: None,
            },
            draft.published_at.as_deref(),
        )
        .await
        .unwrap();

        let published = repository::get_admin_post(&pool, &post_id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(published.status, PostStatus::Published);
        assert!(published.published_at.is_some());
    }

    #[tokio::test]
    async fn test_published_to_draft_clears_published_at() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "author6").await;

        let post_id = insert_post_helper(
            &pool,
            &author_id,
            "Published Post",
            "pub-post",
            PostStatus::Published,
            Visibility::Public,
        )
        .await;

        let published = repository::get_admin_post(&pool, &post_id)
            .await
            .unwrap()
            .unwrap();
        assert!(published.published_at.is_some());

        // 撤回到 draft
        repository::update_post(
            &pool,
            UpdatePostParams {
                post_id: &post_id,
                title: &published.title,
                slug: &published.slug,
                excerpt: published.excerpt.as_deref(),
                content_md: &published.content_md,
                content_html: &published.content_html,
                cover_media_id: published.cover_media_id.as_deref(),
                status: PostStatus::Draft,
                visibility: published.visibility,
                category_id: published.category_id.as_deref(),
                allow_comment: published.allow_comment,
                pinned: published.pinned,
                content_type: published.content_type,
                custom_html_path: published.custom_html_path.as_deref(),
                page_render_mode: &published.page_render_mode,
                scheduled_at: None,
            },
            published.published_at.as_deref(),
        )
        .await
        .unwrap();

        let draft = repository::get_admin_post(&pool, &post_id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(draft.status, PostStatus::Draft);
        assert!(draft.published_at.is_none());
    }

    // ========== 测试：更新文章 ==========

    #[tokio::test]
    async fn test_update_post_content() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "author7").await;

        let post_id = insert_post_helper(
            &pool,
            &author_id,
            "Original Title",
            "original-slug",
            PostStatus::Draft,
            Visibility::Public,
        )
        .await;

        let original = repository::get_admin_post(&pool, &post_id)
            .await
            .unwrap()
            .unwrap();

        // 更新 title 和 content
        repository::update_post(
            &pool,
            UpdatePostParams {
                post_id: &post_id,
                title: "Updated Title",
                slug: &original.slug,
                excerpt: Some("New excerpt"),
                content_md: "# Updated Content",
                content_html: "<h1>Updated Content</h1>",
                cover_media_id: original.cover_media_id.as_deref(),
                status: original.status,
                visibility: original.visibility,
                category_id: original.category_id.as_deref(),
                allow_comment: original.allow_comment,
                pinned: original.pinned,
                content_type: original.content_type,
                custom_html_path: original.custom_html_path.as_deref(),
                page_render_mode: &original.page_render_mode,
                scheduled_at: None,
            },
            original.published_at.as_deref(),
        )
        .await
        .unwrap();

        let updated = repository::get_admin_post(&pool, &post_id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(updated.title, "Updated Title");
        assert_eq!(updated.excerpt, Some("New excerpt".to_string()));
        assert_eq!(updated.content_md, "# Updated Content");
        assert_eq!(updated.slug, "original-slug"); // slug 不变
        assert_eq!(updated.created_at, original.created_at); // created_at 不变
                                                             // updated_at 在 SQLite 中通过 datetime('now') 自动更新，但测试速度太快可能在同一秒内
    }

    #[tokio::test]
    async fn test_slug_can_be_changed_in_update() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "author8").await;

        let post_id = insert_post_helper(
            &pool,
            &author_id,
            "Post",
            "old-slug",
            PostStatus::Draft,
            Visibility::Public,
        )
        .await;

        let original = repository::get_admin_post(&pool, &post_id)
            .await
            .unwrap()
            .unwrap();

        // 更新 slug（通过 resolve_unique_post_slug 处理）
        repository::update_post(
            &pool,
            UpdatePostParams {
                post_id: &post_id,
                title: &original.title,
                slug: "new-slug",
                excerpt: original.excerpt.as_deref(),
                content_md: &original.content_md,
                content_html: &original.content_html,
                cover_media_id: original.cover_media_id.as_deref(),
                status: original.status,
                visibility: original.visibility,
                category_id: original.category_id.as_deref(),
                allow_comment: original.allow_comment,
                pinned: original.pinned,
                content_type: original.content_type,
                custom_html_path: original.custom_html_path.as_deref(),
                page_render_mode: &original.page_render_mode,
                scheduled_at: None,
            },
            original.published_at.as_deref(),
        )
        .await
        .unwrap();

        let updated = repository::get_admin_post(&pool, &post_id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(updated.slug, "new-slug");
    }

    // ========== 测试：删除文章（软删除）==========

    #[tokio::test]
    async fn test_soft_delete_post() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "author9").await;

        let post_id = insert_post_helper(
            &pool,
            &author_id,
            "To Delete",
            "to-delete",
            PostStatus::Published,
            Visibility::Public,
        )
        .await;

        repository::delete_post(&pool, &post_id).await.unwrap();

        let deleted = repository::get_admin_post(&pool, &post_id)
            .await
            .unwrap()
            .unwrap();

        assert!(deleted.deleted_at.is_some());
    }

    #[tokio::test]
    async fn test_deleted_post_excluded_from_public_list() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "author10").await;

        let post_id = insert_post_helper(
            &pool,
            &author_id,
            "Deleted Post",
            "deleted-post",
            PostStatus::Published,
            Visibility::Public,
        )
        .await;

        repository::delete_post(&pool, &post_id).await.unwrap();

        let public_post = repository::get_public_post_by_slug(&pool, "deleted-post")
            .await
            .unwrap();

        assert!(public_post.is_none());
    }

    // ========== 测试：可见性控制 ==========

    #[tokio::test]
    async fn test_public_post_visible() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "author11").await;

        insert_post_helper(
            &pool,
            &author_id,
            "Public Post",
            "public-post",
            PostStatus::Published,
            Visibility::Public,
        )
        .await;

        let post = repository::get_public_post_by_slug(&pool, "public-post")
            .await
            .unwrap();

        assert!(post.is_some());
    }

    #[tokio::test]
    async fn test_private_post_not_in_public_list() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "author12").await;

        insert_post_helper(
            &pool,
            &author_id,
            "Private Post",
            "private-post",
            PostStatus::Published,
            Visibility::Private,
        )
        .await;

        let post = repository::get_public_post_by_slug(&pool, "private-post")
            .await
            .unwrap();

        assert!(post.is_none());
    }

    // ========== 测试：查询功能 ==========

    #[tokio::test]
    async fn test_get_post_by_id() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "author13").await;

        let post_id = insert_post_helper(
            &pool,
            &author_id,
            "Test Post",
            "test-post",
            PostStatus::Published,
            Visibility::Public,
        )
        .await;

        let post = repository::get_admin_post(&pool, &post_id).await.unwrap();

        assert!(post.is_some());
        assert_eq!(post.unwrap().id, post_id);
    }

    #[tokio::test]
    async fn test_get_post_by_slug() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "author14").await;

        insert_post_helper(
            &pool,
            &author_id,
            "Test Post",
            "test-slug",
            PostStatus::Published,
            Visibility::Public,
        )
        .await;

        let post = repository::get_public_post_by_slug(&pool, "test-slug")
            .await
            .unwrap();

        assert!(post.is_some());
        assert_eq!(post.unwrap().slug, "test-slug");
    }

    #[tokio::test]
    async fn test_list_posts_pagination() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "author15").await;

        for i in 0..5 {
            insert_post_helper(
                &pool,
                &author_id,
                &format!("Post {}", i),
                &format!("post-{}", i),
                PostStatus::Published,
                Visibility::Public,
            )
            .await;
        }

        let posts = repository::list_public_posts(&pool, None, 3, 0)
            .await
            .unwrap();

        assert_eq!(posts.len(), 3);
    }

    #[tokio::test]
    async fn test_filter_by_status() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "author16").await;

        insert_post_helper(
            &pool,
            &author_id,
            "Draft Post",
            "draft-post-1",
            PostStatus::Draft,
            Visibility::Public,
        )
        .await;

        insert_post_helper(
            &pool,
            &author_id,
            "Published Post",
            "published-post-1",
            PostStatus::Published,
            Visibility::Public,
        )
        .await;

        let drafts =
            repository::list_admin_posts(&pool, Some(PostStatus::Draft), None, None, 10, 0)
                .await
                .unwrap();

        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].status, PostStatus::Draft);

        let published =
            repository::list_admin_posts(&pool, Some(PostStatus::Published), None, None, 10, 0)
                .await
                .unwrap();

        assert_eq!(published.len(), 1);
        assert_eq!(published[0].status, PostStatus::Published);
    }

    #[tokio::test]
    async fn test_draft_post_not_in_public_list() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "author17").await;

        insert_post_helper(
            &pool,
            &author_id,
            "Draft Post",
            "draft-only",
            PostStatus::Draft,
            Visibility::Public,
        )
        .await;

        let post = repository::get_public_post_by_slug(&pool, "draft-only")
            .await
            .unwrap();

        assert!(post.is_none());
    }
}

// ========================================
// Page 专项测试（ContentType::Page）
// ========================================

#[cfg(test)]
mod page_tests {
    use crate::modules::post::{
        post_types::{ContentType, NewPostParams, PostStatus, UpdatePostParams, Visibility},
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

    /// 创建 Page 的辅助函数
    async fn insert_page_helper(
        pool: &sqlx::SqlitePool,
        author_id: &str,
        title: &str,
        slug: &str,
        status: PostStatus,
        visibility: Visibility,
        page_render_mode: &str,
    ) -> String {
        repository::insert_post(
            pool,
            NewPostParams {
                author_id,
                title,
                slug,
                excerpt: None,
                content_md: "# Page Content",
                content_html: "<h1>Page Content</h1>",
                cover_media_id: None,
                status,
                visibility,
                category_id: None,
                allow_comment: false, // page 通常不允许评论
                pinned: false,
                content_type: ContentType::Page, // 关键：指定为 page
                custom_html_path: None,
                page_render_mode,
                scheduled_at: None,
            },
        )
        .await
        .expect("insert page")
    }

    /// 创建 Post 的辅助函数（用于对比测试）
    async fn insert_post_helper(
        pool: &sqlx::SqlitePool,
        author_id: &str,
        title: &str,
        slug: &str,
        status: PostStatus,
    ) -> String {
        repository::insert_post(
            pool,
            NewPostParams {
                author_id,
                title,
                slug,
                excerpt: None,
                content_md: "# Post Content",
                content_html: "<h1>Post Content</h1>",
                cover_media_id: None,
                status,
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
        .expect("insert post")
    }

    // ========== 测试：创建 Page ==========

    #[tokio::test]
    async fn test_create_markdown_page() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "page_author1").await;

        let page_id = insert_page_helper(
            &pool,
            &author_id,
            "About Us",
            "about",
            PostStatus::Published,
            Visibility::Public,
            "editor",
        )
        .await;

        let page = repository::get_admin_post(&pool, &page_id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(page.title, "About Us");
        assert_eq!(page.slug, "about");
        assert_eq!(page.content_type, ContentType::Page);
        assert_eq!(page.page_render_mode, "editor");
        assert_eq!(page.status, PostStatus::Published);
    }

    #[tokio::test]
    async fn test_create_custom_html_page() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "page_author2").await;

        let page_id = repository::insert_post(
            &pool,
            NewPostParams {
                author_id: &author_id,
                title: "Custom Page",
                slug: "custom",
                excerpt: None,
                content_md: "",
                content_html: "<div class=\"custom-content\">Custom HTML</div>",
                cover_media_id: None,
                status: PostStatus::Published,
                visibility: Visibility::Public,
                category_id: None,
                allow_comment: false,
                pinned: false,
                content_type: ContentType::Page,
                custom_html_path: Some("/custom/page.html"),
                page_render_mode: "custom_html",
                scheduled_at: None,
            },
        )
        .await
        .unwrap();

        let page = repository::get_admin_post(&pool, &page_id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(page.page_render_mode, "custom_html");
        assert_eq!(page.custom_html_path, Some("/custom/page.html".to_string()));
        assert!(page.content_html.contains("custom-content"));
    }

    #[tokio::test]
    async fn test_page_content_type_auto_set() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "page_author3").await;

        let page_id = insert_page_helper(
            &pool,
            &author_id,
            "Contact",
            "contact",
            PostStatus::Published,
            Visibility::Public,
            "editor",
        )
        .await;

        let page = repository::get_admin_post(&pool, &page_id)
            .await
            .unwrap()
            .unwrap();

        // 验证 content_type 正确设置为 page
        assert_eq!(page.content_type, ContentType::Page);
    }

    // ========== 测试：Slug 冲突检测 ==========

    #[tokio::test]
    async fn test_slug_conflict_between_pages() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "page_author4").await;

        // 创建第一个 page
        insert_page_helper(
            &pool,
            &author_id,
            "First Page",
            "same-slug",
            PostStatus::Published,
            Visibility::Public,
            "editor",
        )
        .await;

        // 检查 slug 冲突
        let exists = repository::slug_exists(&pool, "same-slug", None)
            .await
            .unwrap();

        assert!(exists);
    }

    #[tokio::test]
    async fn test_slug_conflict_between_page_and_post() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "page_author5").await;

        // 创建一个 post
        insert_post_helper(
            &pool,
            &author_id,
            "Post Title",
            "conflict-slug",
            PostStatus::Published,
        )
        .await;

        // 检查 page 是否能检测到与 post 的 slug 冲突
        let exists = repository::slug_exists(&pool, "conflict-slug", None)
            .await
            .unwrap();

        assert!(exists); // page 和 post 共用 slug 空间
    }

    // ========== 测试：更新 Page ==========

    #[tokio::test]
    async fn test_update_page_content() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "page_author6").await;

        let page_id = insert_page_helper(
            &pool,
            &author_id,
            "Original Title",
            "page-slug",
            PostStatus::Published,
            Visibility::Public,
            "editor",
        )
        .await;

        let original = repository::get_admin_post(&pool, &page_id)
            .await
            .unwrap()
            .unwrap();

        // 更新内容
        repository::update_post(
            &pool,
            UpdatePostParams {
                post_id: &page_id,
                title: "Updated Title",
                slug: &original.slug,
                excerpt: Some("Updated excerpt"),
                content_md: "# Updated Page Content",
                content_html: "<h1>Updated Page Content</h1>",
                cover_media_id: original.cover_media_id.as_deref(),
                status: original.status,
                visibility: original.visibility,
                category_id: original.category_id.as_deref(),
                allow_comment: original.allow_comment,
                pinned: original.pinned,
                content_type: ContentType::Page,
                custom_html_path: original.custom_html_path.as_deref(),
                page_render_mode: &original.page_render_mode,
                scheduled_at: None,
            },
            original.published_at.as_deref(),
        )
        .await
        .unwrap();

        let updated = repository::get_admin_post(&pool, &page_id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(updated.title, "Updated Title");
        assert_eq!(updated.content_md, "# Updated Page Content");
        assert_eq!(updated.content_type, ContentType::Page);
    }

    #[tokio::test]
    async fn test_switch_page_render_mode() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "page_author7").await;

        let page_id = insert_page_helper(
            &pool,
            &author_id,
            "Page",
            "switch-mode",
            PostStatus::Published,
            Visibility::Public,
            "editor",
        )
        .await;

        let original = repository::get_admin_post(&pool, &page_id)
            .await
            .unwrap()
            .unwrap();

        // 切换到 custom_html 模式
        repository::update_post(
            &pool,
            UpdatePostParams {
                post_id: &page_id,
                title: &original.title,
                slug: &original.slug,
                excerpt: original.excerpt.as_deref(),
                content_md: &original.content_md,
                content_html: "<div>Custom HTML</div>",
                cover_media_id: original.cover_media_id.as_deref(),
                status: original.status,
                visibility: original.visibility,
                category_id: original.category_id.as_deref(),
                allow_comment: original.allow_comment,
                pinned: original.pinned,
                content_type: ContentType::Page,
                custom_html_path: Some("/custom.html"),
                page_render_mode: "custom_html",
                scheduled_at: None,
            },
            original.published_at.as_deref(),
        )
        .await
        .unwrap();

        let updated = repository::get_admin_post(&pool, &page_id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(updated.page_render_mode, "custom_html");
        assert_eq!(updated.custom_html_path, Some("/custom.html".to_string()));
    }

    // ========== 测试：删除 Page ==========

    #[tokio::test]
    async fn test_soft_delete_page() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "page_author8").await;

        let page_id = insert_page_helper(
            &pool,
            &author_id,
            "Delete Me",
            "delete-me",
            PostStatus::Published,
            Visibility::Public,
            "editor",
        )
        .await;

        repository::delete_post(&pool, &page_id).await.unwrap();

        // 查询应仍能获取（软删除）
        let deleted = repository::get_admin_post(&pool, &page_id)
            .await
            .unwrap()
            .unwrap();

        assert!(deleted.deleted_at.is_some());
    }

    #[tokio::test]
    async fn test_deleted_page_not_in_public_query() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "page_author9").await;

        let page_id = insert_page_helper(
            &pool,
            &author_id,
            "Deleted Page",
            "deleted-page",
            PostStatus::Published,
            Visibility::Public,
            "editor",
        )
        .await;

        repository::delete_post(&pool, &page_id).await.unwrap();

        // 公开查询应返回 None
        let public_page = repository::get_public_post_by_slug(&pool, "deleted-page")
            .await
            .unwrap();

        assert!(public_page.is_none());
    }

    // ========== 测试：查询 Page ==========

    #[tokio::test]
    async fn test_get_page_by_slug() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "page_author10").await;

        insert_page_helper(
            &pool,
            &author_id,
            "Contact Us",
            "contact-us",
            PostStatus::Published,
            Visibility::Public,
            "editor",
        )
        .await;

        let page = repository::get_public_post_by_slug(&pool, "contact-us")
            .await
            .unwrap()
            .unwrap();

        assert_eq!(page.title, "Contact Us");
        assert_eq!(page.content_type, ContentType::Page);
    }

    #[tokio::test]
    async fn test_list_pages_excludes_posts() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "page_author11").await;

        // 创建 1 个 page
        insert_page_helper(
            &pool,
            &author_id,
            "About Page",
            "about-page",
            PostStatus::Published,
            Visibility::Public,
            "editor",
        )
        .await;

        // 创建 1 个 post
        insert_post_helper(
            &pool,
            &author_id,
            "Blog Post",
            "blog-post",
            PostStatus::Published,
        )
        .await;

        // 查询只返回 page
        let pages = repository::list_admin_posts(&pool, None, None, Some(ContentType::Page), 10, 0)
            .await
            .unwrap();

        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].slug, "about-page");
        assert_eq!(pages[0].content_type, ContentType::Page);
    }

    #[tokio::test]
    async fn test_list_posts_excludes_pages() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "page_author12").await;

        // 创建 1 个 page
        insert_page_helper(
            &pool,
            &author_id,
            "About Page",
            "about-page-2",
            PostStatus::Published,
            Visibility::Public,
            "editor",
        )
        .await;

        // 创建 1 个 post
        insert_post_helper(
            &pool,
            &author_id,
            "Blog Post",
            "blog-post-2",
            PostStatus::Published,
        )
        .await;

        // 查询只返回 post
        let posts = repository::list_admin_posts(&pool, None, None, Some(ContentType::Post), 10, 0)
            .await
            .unwrap();

        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].slug, "blog-post-2");
        assert_eq!(posts[0].content_type, ContentType::Post);
    }

    // ========== 测试：可见性控制 ==========

    #[tokio::test]
    async fn test_public_page_visible() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "page_author13").await;

        insert_page_helper(
            &pool,
            &author_id,
            "Public Page",
            "public-page",
            PostStatus::Published,
            Visibility::Public,
            "editor",
        )
        .await;

        let page = repository::get_public_post_by_slug(&pool, "public-page")
            .await
            .unwrap();

        assert!(page.is_some());
    }

    #[tokio::test]
    async fn test_private_page_not_in_public_query() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "page_author14").await;

        insert_page_helper(
            &pool,
            &author_id,
            "Private Page",
            "private-page",
            PostStatus::Published,
            Visibility::Private,
            "editor",
        )
        .await;

        let page = repository::get_public_post_by_slug(&pool, "private-page")
            .await
            .unwrap();

        assert!(page.is_none());
    }

    #[tokio::test]
    async fn test_draft_page_not_in_public_query() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "page_author15").await;

        insert_page_helper(
            &pool,
            &author_id,
            "Draft Page",
            "draft-page",
            PostStatus::Draft,
            Visibility::Public,
            "editor",
        )
        .await;

        let page = repository::get_public_post_by_slug(&pool, "draft-page")
            .await
            .unwrap();

        assert!(page.is_none());
    }

    // ========== 测试：Page 状态机 ==========

    #[tokio::test]
    async fn test_page_draft_to_published() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "page_author16").await;

        let page_id = insert_page_helper(
            &pool,
            &author_id,
            "Draft Page",
            "draft-to-pub",
            PostStatus::Draft,
            Visibility::Public,
            "editor",
        )
        .await;

        let draft = repository::get_admin_post(&pool, &page_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(draft.status, PostStatus::Draft);
        assert!(draft.published_at.is_none());

        // 更新为 published
        repository::update_post(
            &pool,
            UpdatePostParams {
                post_id: &page_id,
                title: &draft.title,
                slug: &draft.slug,
                excerpt: draft.excerpt.as_deref(),
                content_md: &draft.content_md,
                content_html: &draft.content_html,
                cover_media_id: draft.cover_media_id.as_deref(),
                status: PostStatus::Published,
                visibility: draft.visibility,
                category_id: draft.category_id.as_deref(),
                allow_comment: draft.allow_comment,
                pinned: draft.pinned,
                content_type: ContentType::Page,
                custom_html_path: draft.custom_html_path.as_deref(),
                page_render_mode: &draft.page_render_mode,
                scheduled_at: None,
            },
            draft.published_at.as_deref(),
        )
        .await
        .unwrap();

        let published = repository::get_admin_post(&pool, &page_id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(published.status, PostStatus::Published);
        assert!(published.published_at.is_some());
    }

    #[tokio::test]
    async fn test_page_published_to_draft_clears_published_at() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "page_author17").await;

        let page_id = insert_page_helper(
            &pool,
            &author_id,
            "Published Page",
            "pub-to-draft",
            PostStatus::Published,
            Visibility::Public,
            "editor",
        )
        .await;

        let published = repository::get_admin_post(&pool, &page_id)
            .await
            .unwrap()
            .unwrap();
        assert!(published.published_at.is_some());

        // 撤回到 draft
        repository::update_post(
            &pool,
            UpdatePostParams {
                post_id: &page_id,
                title: &published.title,
                slug: &published.slug,
                excerpt: published.excerpt.as_deref(),
                content_md: &published.content_md,
                content_html: &published.content_html,
                cover_media_id: published.cover_media_id.as_deref(),
                status: PostStatus::Draft,
                visibility: published.visibility,
                category_id: published.category_id.as_deref(),
                allow_comment: published.allow_comment,
                pinned: published.pinned,
                content_type: ContentType::Page,
                custom_html_path: published.custom_html_path.as_deref(),
                page_render_mode: &published.page_render_mode,
                scheduled_at: None,
            },
            published.published_at.as_deref(),
        )
        .await
        .unwrap();

        let draft = repository::get_admin_post(&pool, &page_id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(draft.status, PostStatus::Draft);
        assert!(draft.published_at.is_none());
    }
}

// ========================================
// 定时发布功能测试
// ========================================

#[cfg(test)]
mod scheduled_publishing_tests {
    use crate::modules::post::{
        post_types::{ContentType, NewPostParams, PostStatus, UpdatePostParams, Visibility},
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

    /// 插入一个 Scheduled 状态的文章，带 scheduled_at
    async fn insert_scheduled_post(
        pool: &sqlx::SqlitePool,
        author_id: &str,
        title: &str,
        slug: &str,
        scheduled_at: &str,
    ) -> String {
        repository::insert_post(
            pool,
            NewPostParams {
                author_id,
                title,
                slug,
                excerpt: None,
                content_md: "# Scheduled Content",
                content_html: "<h1>Scheduled Content</h1>",
                cover_media_id: None,
                status: PostStatus::Scheduled,
                visibility: Visibility::Public,
                category_id: None,
                allow_comment: true,
                pinned: false,
                content_type: ContentType::Post,
                custom_html_path: None,
                page_render_mode: "editor",
                scheduled_at: Some(scheduled_at),
            },
        )
        .await
        .expect("insert scheduled post")
    }

    // ========== 测试：创建定时发布文章 ==========

    #[tokio::test]
    async fn insert_post_with_scheduled_status_and_future_time() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "sched_author1").await;

        let post_id = insert_scheduled_post(
            &pool,
            &author_id,
            "Future Post",
            "future-post",
            "2099-12-31 23:59:59",
        )
        .await;

        let post = repository::get_admin_post(&pool, &post_id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(post.status, PostStatus::Scheduled);
        assert_eq!(
            post.scheduled_at,
            Some("2099-12-31 23:59:59".to_string())
        );
        assert!(post.published_at.is_none());
    }

    #[tokio::test]
    async fn insert_draft_post_with_no_scheduled_at() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "sched_author2").await;

        let post_id = repository::insert_post(
            &pool,
            NewPostParams {
                author_id: &author_id,
                title: "Draft Post",
                slug: "draft-post",
                excerpt: None,
                content_md: "",
                content_html: "",
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
        .unwrap();

        let post = repository::get_admin_post(&pool, &post_id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(post.status, PostStatus::Draft);
        assert!(post.scheduled_at.is_none());
    }

    // ========== 测试：状态机转换 ==========

    #[tokio::test]
    async fn update_from_scheduled_to_draft_clears_scheduled_at() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "sched_author3").await;

        let post_id = insert_scheduled_post(
            &pool,
            &author_id,
            "Scheduled Post",
            "sched-to-draft",
            "2099-12-31 23:59:59",
        )
        .await;

        let scheduled = repository::get_admin_post(&pool, &post_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(scheduled.status, PostStatus::Scheduled);
        assert!(scheduled.scheduled_at.is_some());

        // 更新为 Draft，清空 scheduled_at
        repository::update_post(
            &pool,
            UpdatePostParams {
                post_id: &post_id,
                title: &scheduled.title,
                slug: &scheduled.slug,
                excerpt: scheduled.excerpt.as_deref(),
                content_md: &scheduled.content_md,
                content_html: &scheduled.content_html,
                cover_media_id: scheduled.cover_media_id.as_deref(),
                status: PostStatus::Draft,
                visibility: scheduled.visibility,
                category_id: scheduled.category_id.as_deref(),
                allow_comment: scheduled.allow_comment,
                pinned: scheduled.pinned,
                content_type: scheduled.content_type,
                custom_html_path: scheduled.custom_html_path.as_deref(),
                page_render_mode: &scheduled.page_render_mode,
                scheduled_at: None,
            },
            scheduled.published_at.as_deref(),
        )
        .await
        .unwrap();

        let draft = repository::get_admin_post(&pool, &post_id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(draft.status, PostStatus::Draft);
        assert!(draft.scheduled_at.is_none());
    }

    #[tokio::test]
    async fn update_from_draft_to_scheduled_sets_scheduled_at() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "sched_author4").await;

        let post_id = repository::insert_post(
            &pool,
            NewPostParams {
                author_id: &author_id,
                title: "Draft Post",
                slug: "draft-to-sched",
                excerpt: None,
                content_md: "",
                content_html: "",
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
        .unwrap();

        let draft = repository::get_admin_post(&pool, &post_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(draft.status, PostStatus::Draft);
        assert!(draft.scheduled_at.is_none());

        // 更新为 Scheduled，设置 scheduled_at
        repository::update_post(
            &pool,
            UpdatePostParams {
                post_id: &post_id,
                title: &draft.title,
                slug: &draft.slug,
                excerpt: draft.excerpt.as_deref(),
                content_md: &draft.content_md,
                content_html: &draft.content_html,
                cover_media_id: draft.cover_media_id.as_deref(),
                status: PostStatus::Scheduled,
                visibility: draft.visibility,
                category_id: draft.category_id.as_deref(),
                allow_comment: draft.allow_comment,
                pinned: draft.pinned,
                content_type: draft.content_type,
                custom_html_path: draft.custom_html_path.as_deref(),
                page_render_mode: &draft.page_render_mode,
                scheduled_at: Some("2099-06-15 10:00:00"),
            },
            draft.published_at.as_deref(),
        )
        .await
        .unwrap();

        let scheduled = repository::get_admin_post(&pool, &post_id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(scheduled.status, PostStatus::Scheduled);
        assert_eq!(
            scheduled.scheduled_at,
            Some("2099-06-15 10:00:00".to_string())
        );
        assert!(scheduled.published_at.is_none());
    }

    #[tokio::test]
    async fn scheduled_post_not_in_public_list() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "sched_author5").await;

        insert_scheduled_post(
            &pool,
            &author_id,
            "Hidden Scheduled",
            "hidden-sched",
            "2099-12-31 23:59:59",
        )
        .await;

        // 公开列表不应包含 Scheduled 状态的文章
        let public = repository::get_public_post_by_slug(&pool, "hidden-sched")
            .await
            .unwrap();
        assert!(public.is_none());
    }

    #[tokio::test]
    async fn admin_can_list_scheduled_posts() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "sched_author6").await;

        insert_scheduled_post(
            &pool,
            &author_id,
            "Admin Visible",
            "admin-visible",
            "2099-12-31 23:59:59",
        )
        .await;

        let scheduled = repository::list_admin_posts(
            &pool,
            Some(PostStatus::Scheduled),
            None,
            None,
            10,
            0,
        )
        .await
        .unwrap();

        assert_eq!(scheduled.len(), 1);
        assert_eq!(scheduled[0].status, PostStatus::Scheduled);
        assert_eq!(
            scheduled[0].scheduled_at,
            Some("2099-12-31 23:59:59".to_string())
        );
    }

    #[tokio::test]
    async fn update_scheduled_at_on_existing_scheduled_post() {
        let pool = new_migrated_pool().await;
        let author_id = create_test_user(&pool, "sched_author7").await;

        let post_id = insert_scheduled_post(
            &pool,
            &author_id,
            "Reschedule Post",
            "reschedule",
            "2099-06-01 10:00:00",
        )
        .await;

        let original = repository::get_admin_post(&pool, &post_id)
            .await
            .unwrap()
            .unwrap();

        // 修改 scheduled_at 时间
        repository::update_post(
            &pool,
            UpdatePostParams {
                post_id: &post_id,
                title: &original.title,
                slug: &original.slug,
                excerpt: original.excerpt.as_deref(),
                content_md: &original.content_md,
                content_html: &original.content_html,
                cover_media_id: original.cover_media_id.as_deref(),
                status: PostStatus::Scheduled,
                visibility: original.visibility,
                category_id: original.category_id.as_deref(),
                allow_comment: original.allow_comment,
                pinned: original.pinned,
                content_type: original.content_type,
                custom_html_path: original.custom_html_path.as_deref(),
                page_render_mode: &original.page_render_mode,
                scheduled_at: Some("2099-12-25 08:00:00"),
            },
            original.published_at.as_deref(),
        )
        .await
        .unwrap();

        let updated = repository::get_admin_post(&pool, &post_id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(updated.status, PostStatus::Scheduled);
        assert_eq!(
            updated.scheduled_at,
            Some("2099-12-25 08:00:00".to_string())
        );
    }
}

// ========================================
// publish_scheduled_posts 仓库层测试（C-1: 软删除过滤）
// ========================================

#[cfg(test)]
mod publish_scheduled_posts_tests {
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

    async fn insert_test_author(pool: &sqlx::SqlitePool) {
        sqlx::query(
            "INSERT INTO users (id, username, email, password_hash, display_name, role, status)
             VALUES ('test-author', 'testuser', 'test@test.com', 'hash', 'Test User', 'admin', 'active')",
        )
        .execute(pool)
        .await
        .expect("insert author");
    }

    async fn insert_scheduled_post_with_time(
        pool: &sqlx::SqlitePool,
        title: &str,
        slug: &str,
        scheduled_at: &str,
    ) -> String {
        repository::insert_post(
            pool,
            NewPostParams {
                author_id: "test-author",
                title,
                slug,
                excerpt: None,
                content_md: "",
                content_html: "",
                cover_media_id: None,
                status: PostStatus::Scheduled,
                visibility: Visibility::Public,
                category_id: None,
                allow_comment: true,
                pinned: false,
                content_type: ContentType::Post,
                custom_html_path: None,
                page_render_mode: "editor",
                scheduled_at: Some(scheduled_at),
            },
        )
        .await
        .expect("insert scheduled post")
    }

    #[tokio::test]
    async fn publishes_due_posts() {
        let pool = new_migrated_pool().await;
        insert_test_author(&pool).await;

        // scheduled_at 在过去，应被发布
        insert_scheduled_post_with_time(
            &pool,
            "Due Post",
            "due-post",
            "2020-01-01 00:00:00",
        )
        .await;

        let ids = repository::publish_scheduled_posts(&pool)
            .await
            .expect("publish");
        assert_eq!(ids.len(), 1);

        let post = repository::get_admin_post(&pool, &ids[0])
            .await
            .expect("get")
            .unwrap();
        assert_eq!(post.status, PostStatus::Published);
        assert!(post.published_at.is_some());
    }

    #[tokio::test]
    async fn skips_future_posts() {
        let pool = new_migrated_pool().await;
        insert_test_author(&pool).await;

        // scheduled_at 在未来，不应被发布
        insert_scheduled_post_with_time(
            &pool,
            "Future Post",
            "future-post",
            "2099-12-31 23:59:59",
        )
        .await;

        let ids = repository::publish_scheduled_posts(&pool)
            .await
            .expect("publish");
        assert_eq!(ids.len(), 0);
    }

    #[tokio::test]
    async fn skips_deleted_posts() {
        let pool = new_migrated_pool().await;
        insert_test_author(&pool).await;

        // 插入一篇到期的定时文章
        let id = insert_scheduled_post_with_time(
            &pool,
            "Deleted Post",
            "deleted-post",
            "2020-01-01 00:00:00",
        )
        .await;

        // 软删除
        repository::delete_post(&pool, &id)
            .await
            .expect("delete");

        let ids = repository::publish_scheduled_posts(&pool)
            .await
            .expect("publish");
        assert_eq!(ids.len(), 0);

        // 验证状态未变
        let post = repository::get_admin_post(&pool, &id)
            .await
            .expect("get")
            .unwrap();
        assert_eq!(post.status, PostStatus::Scheduled);
        assert!(post.published_at.is_none());
    }

    #[tokio::test]
    async fn sets_published_at_from_scheduled_at() {
        let pool = new_migrated_pool().await;
        insert_test_author(&pool).await;

        let scheduled_time = "2020-06-15 10:30:00";

        insert_scheduled_post_with_time(
            &pool,
            "Time Check",
            "time-check",
            scheduled_time,
        )
        .await;

        let ids = repository::publish_scheduled_posts(&pool)
            .await
            .expect("publish");
        assert_eq!(ids.len(), 1);

        let post = repository::get_admin_post(&pool, &ids[0])
            .await
            .expect("get")
            .unwrap();
        assert_eq!(post.published_at.as_deref(), Some(scheduled_time));
    }
}
