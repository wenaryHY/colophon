#[cfg(test)]
mod archive_tests {
    use crate::modules::post::repository::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;

    async fn setup_test_db() -> sqlx::SqlitePool {
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

    #[tokio::test]
    async fn list_posts_by_tag_slug_returns_matching_posts() {
        let pool = setup_test_db().await;

        // 创建用户
        let user_id = create_test_user(&pool, "author").await;

        // 创建标签
        let tag_id = crate::modules::tag::repository::insert_tag(&pool, "Rust", "rust")
            .await
            .unwrap();

        // 创建文章
        let post1_id = insert_post(
            &pool,
            crate::modules::post::post_types::NewPostParams {
                author_id: &user_id,
                title: "Post 1",
                slug: "post-1",
                excerpt: Some("Excerpt 1"),
                content_md: "Content 1",
                content_html: "<p>Content 1</p>",
                cover_media_id: None,
                status: crate::modules::post::post_types::PostStatus::Published,
                visibility: crate::modules::post::post_types::Visibility::Public,
                category_id: None,
                allow_comment: true,
                pinned: false,
                content_type: crate::modules::post::post_types::ContentType::Post,
                custom_html_path: None,
                page_render_mode: "editor",
            },
        )
        .await
        .unwrap();

        let post2_id = insert_post(
            &pool,
            crate::modules::post::post_types::NewPostParams {
                author_id: &user_id,
                title: "Post 2",
                slug: "post-2",
                excerpt: Some("Excerpt 2"),
                content_md: "Content 2",
                content_html: "<p>Content 2</p>",
                cover_media_id: None,
                status: crate::modules::post::post_types::PostStatus::Published,
                visibility: crate::modules::post::post_types::Visibility::Public,
                category_id: None,
                allow_comment: true,
                pinned: false,
                content_type: crate::modules::post::post_types::ContentType::Post,
                custom_html_path: None,
                page_render_mode: "editor",
            },
        )
        .await
        .unwrap();

        // 关联标签
        replace_tags(&pool, &post1_id, &[tag_id.clone()])
            .await
            .unwrap();
        replace_tags(&pool, &post2_id, &[tag_id.clone()])
            .await
            .unwrap();

        // 查询
        let posts = list_posts_by_tag_slug(&pool, "rust", 1, 10).await.unwrap();
        assert_eq!(posts.len(), 2);

        let count = count_posts_by_tag_slug(&pool, "rust").await.unwrap();
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn list_posts_by_tag_slug_excludes_draft_posts() {
        let pool = setup_test_db().await;

        let user_id = create_test_user(&pool, "author").await;

        let tag_id = crate::modules::tag::repository::insert_tag(&pool, "Draft", "draft")
            .await
            .unwrap();

        // 创建草稿
        let post_id = insert_post(
            &pool,
            crate::modules::post::post_types::NewPostParams {
                author_id: &user_id,
                title: "Draft",
                slug: "draft",
                excerpt: Some("Draft"),
                content_md: "Draft",
                content_html: "<p>Draft</p>",
                cover_media_id: None,
                status: crate::modules::post::post_types::PostStatus::Draft,
                visibility: crate::modules::post::post_types::Visibility::Public,
                category_id: None,
                allow_comment: true,
                pinned: false,
                content_type: crate::modules::post::post_types::ContentType::Post,
                custom_html_path: None,
                page_render_mode: "editor",
            },
        )
        .await
        .unwrap();

        replace_tags(&pool, &post_id, &[tag_id]).await.unwrap();

        let posts = list_posts_by_tag_slug(&pool, "draft", 1, 10)
            .await
            .unwrap();
        assert_eq!(posts.len(), 0);

        let count = count_posts_by_tag_slug(&pool, "draft").await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn list_posts_by_tag_slug_respects_pagination() {
        let pool = setup_test_db().await;

        let user_id = create_test_user(&pool, "author").await;

        let tag_id = crate::modules::tag::repository::insert_tag(&pool, "Paginated", "paginated")
            .await
            .unwrap();

        // 创建 5 篇文章
        for i in 1..=5 {
            let post_id = insert_post(
                &pool,
                crate::modules::post::post_types::NewPostParams {
                    author_id: &user_id,
                    title: &format!("Post {}", i),
                    slug: &format!("post-{}", i),
                    excerpt: Some("Excerpt"),
                    content_md: "Content",
                    content_html: "<p>Content</p>",
                    cover_media_id: None,
                    status: crate::modules::post::post_types::PostStatus::Published,
                    visibility: crate::modules::post::post_types::Visibility::Public,
                    category_id: None,
                    allow_comment: true,
                    pinned: false,
                    content_type: crate::modules::post::post_types::ContentType::Post,
                    custom_html_path: None,
                    page_render_mode: "editor",
                },
            )
            .await
            .unwrap();

            replace_tags(&pool, &post_id, &[tag_id.clone()])
                .await
                .unwrap();
        }

        // 第 1 页，每页 3 条
        let page1 = list_posts_by_tag_slug(&pool, "paginated", 1, 3)
            .await
            .unwrap();
        assert_eq!(page1.len(), 3);

        // 第 2 页，每页 3 条
        let page2 = list_posts_by_tag_slug(&pool, "paginated", 2, 3)
            .await
            .unwrap();
        assert_eq!(page2.len(), 2);

        let count = count_posts_by_tag_slug(&pool, "paginated").await.unwrap();
        assert_eq!(count, 5);
    }

    #[tokio::test]
    async fn list_posts_by_category_slug_returns_matching_posts() {
        let pool = setup_test_db().await;

        let user_id = create_test_user(&pool, "author").await;

        let cat_id =
            crate::modules::category::repository::insert_category(&pool, "Tech", "tech", None, None, 0)
                .await
                .unwrap();

        let _post1_id = insert_post(
            &pool,
            crate::modules::post::post_types::NewPostParams {
                author_id: &user_id,
                title: "Post 1",
                slug: "post-1",
                excerpt: Some("Excerpt 1"),
                content_md: "Content 1",
                content_html: "<p>Content 1</p>",
                cover_media_id: None,
                status: crate::modules::post::post_types::PostStatus::Published,
                visibility: crate::modules::post::post_types::Visibility::Public,
                category_id: Some(&cat_id),
                allow_comment: true,
                pinned: false,
                content_type: crate::modules::post::post_types::ContentType::Post,
                custom_html_path: None,
                page_render_mode: "editor",
            },
        )
        .await
        .unwrap();

        let _post2_id = insert_post(
            &pool,
            crate::modules::post::post_types::NewPostParams {
                author_id: &user_id,
                title: "Post 2",
                slug: "post-2",
                excerpt: Some("Excerpt 2"),
                content_md: "Content 2",
                content_html: "<p>Content 2</p>",
                cover_media_id: None,
                status: crate::modules::post::post_types::PostStatus::Published,
                visibility: crate::modules::post::post_types::Visibility::Public,
                category_id: Some(&cat_id),
                allow_comment: true,
                pinned: false,
                content_type: crate::modules::post::post_types::ContentType::Post,
                custom_html_path: None,
                page_render_mode: "editor",
            },
        )
        .await
        .unwrap();

        let posts = list_posts_by_category_slug(&pool, "tech", 1, 10)
            .await
            .unwrap();
        assert_eq!(posts.len(), 2);

        let count = count_posts_by_category_slug(&pool, "tech").await.unwrap();
        assert_eq!(count, 2);
    }

    // ── Task 7: 作者页测试 ──

    #[tokio::test]
    async fn list_posts_by_author_username_returns_matching_posts() {
        let pool = setup_test_db().await;

        // 创建用户
        let user_id = create_test_user(&pool, "john_doe").await;

        // 创建 2 篇作者文章
        let _post1_id = insert_post(
            &pool,
            crate::modules::post::post_types::NewPostParams {
                author_id: &user_id,
                title: "Author Post 1",
                slug: "author-post-1",
                excerpt: Some("Excerpt 1"),
                content_md: "Content 1",
                content_html: "<p>Content 1</p>",
                cover_media_id: None,
                status: crate::modules::post::post_types::PostStatus::Published,
                visibility: crate::modules::post::post_types::Visibility::Public,
                category_id: None,
                allow_comment: true,
                pinned: false,
                content_type: crate::modules::post::post_types::ContentType::Post,
                custom_html_path: None,
                page_render_mode: "editor",
            },
        )
        .await
        .unwrap();

        let _post2_id = insert_post(
            &pool,
            crate::modules::post::post_types::NewPostParams {
                author_id: &user_id,
                title: "Author Post 2",
                slug: "author-post-2",
                excerpt: Some("Excerpt 2"),
                content_md: "Content 2",
                content_html: "<p>Content 2</p>",
                cover_media_id: None,
                status: crate::modules::post::post_types::PostStatus::Published,
                visibility: crate::modules::post::post_types::Visibility::Public,
                category_id: None,
                allow_comment: true,
                pinned: false,
                content_type: crate::modules::post::post_types::ContentType::Post,
                custom_html_path: None,
                page_render_mode: "editor",
            },
        )
        .await
        .unwrap();

        // 查询作者文章
        let posts = list_posts_by_author_username(&pool, "john_doe", 1, 10)
            .await
            .unwrap();
        assert_eq!(posts.len(), 2);

        let count = count_posts_by_author_username(&pool, "john_doe")
            .await
            .unwrap();
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn list_posts_by_author_username_returns_empty_for_nonexistent_user() {
        let pool = setup_test_db().await;

        // 不创建任何用户，查询不存在的用户名
        let posts = list_posts_by_author_username(&pool, "nonexistent_user", 1, 10)
            .await
            .unwrap();
        assert_eq!(posts.len(), 0);

        let count = count_posts_by_author_username(&pool, "nonexistent_user")
            .await
            .unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn list_posts_by_author_username_excludes_draft_posts() {
        let pool = setup_test_db().await;

        let user_id = create_test_user(&pool, "draft_author").await;

        // 创建草稿
        let _draft_id = insert_post(
            &pool,
            crate::modules::post::post_types::NewPostParams {
                author_id: &user_id,
                title: "Draft Post",
                slug: "draft-post",
                excerpt: Some("Draft"),
                content_md: "Draft content",
                content_html: "<p>Draft</p>",
                cover_media_id: None,
                status: crate::modules::post::post_types::PostStatus::Draft,
                visibility: crate::modules::post::post_types::Visibility::Public,
                category_id: None,
                allow_comment: true,
                pinned: false,
                content_type: crate::modules::post::post_types::ContentType::Post,
                custom_html_path: None,
                page_render_mode: "editor",
            },
        )
        .await
        .unwrap();

        // 创建已发布文章
        let _published_id = insert_post(
            &pool,
            crate::modules::post::post_types::NewPostParams {
                author_id: &user_id,
                title: "Published Post",
                slug: "published-post",
                excerpt: Some("Published"),
                content_md: "Published content",
                content_html: "<p>Published</p>",
                cover_media_id: None,
                status: crate::modules::post::post_types::PostStatus::Published,
                visibility: crate::modules::post::post_types::Visibility::Public,
                category_id: None,
                allow_comment: true,
                pinned: false,
                content_type: crate::modules::post::post_types::ContentType::Post,
                custom_html_path: None,
                page_render_mode: "editor",
            },
        )
        .await
        .unwrap();

        let posts = list_posts_by_author_username(&pool, "draft_author", 1, 10)
            .await
            .unwrap();
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].title, "Published Post");
    }

    #[tokio::test]
    async fn list_posts_by_category_slug_excludes_pages() {
        let pool = setup_test_db().await;

        let user_id = create_test_user(&pool, "author").await;

        let cat_id =
            crate::modules::category::repository::insert_category(&pool, "Pages", "pages", None, None, 0)
                .await
                .unwrap();

        // 创建 page（不是 post）
        let _page_id = insert_post(
            &pool,
            crate::modules::post::post_types::NewPostParams {
                author_id: &user_id,
                title: "Page",
                slug: "page",
                excerpt: Some("Page"),
                content_md: "Page",
                content_html: "<p>Page</p>",
                cover_media_id: None,
                status: crate::modules::post::post_types::PostStatus::Published,
                visibility: crate::modules::post::post_types::Visibility::Public,
                category_id: Some(&cat_id),
                allow_comment: false,
                pinned: false,
                content_type: crate::modules::post::post_types::ContentType::Page,
                custom_html_path: None,
                page_render_mode: "editor",
            },
        )
        .await
        .unwrap();

        let posts = list_posts_by_category_slug(&pool, "pages", 1, 10)
            .await
            .unwrap();
        assert_eq!(posts.len(), 0); // pages 不计入

        let count = count_posts_by_category_slug(&pool, "pages").await.unwrap();
        assert_eq!(count, 0);
    }
}
