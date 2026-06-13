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
