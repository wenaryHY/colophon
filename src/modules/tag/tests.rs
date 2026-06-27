#[cfg(test)]
mod tests {
    use super::super::repository::*;
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
    async fn get_by_slug_returns_tag_when_exists() {
        let pool = setup_test_db().await;

        let tag_id = insert_tag(&pool, "Rust", "rust").await.unwrap();

        let result = get_by_slug(&pool, "rust").await.unwrap();
        assert!(result.is_some());
        let tag = result.unwrap();
        assert_eq!(tag.id, tag_id);
        assert_eq!(tag.name, "Rust");
        assert_eq!(tag.slug, "rust");
    }

    #[tokio::test]
    async fn get_by_slug_returns_none_when_not_exists() {
        let pool = setup_test_db().await;

        let result = get_by_slug(&pool, "nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn get_by_slug_ignores_deleted_tags() {
        let pool = setup_test_db().await;

        let tag_id = insert_tag(&pool, "Deleted", "deleted").await.unwrap();
        delete_tag(&pool, &tag_id).await.unwrap();

        let result = get_by_slug(&pool, "deleted").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn get_all_tags_with_count_returns_only_tags_with_posts() {
        let pool = setup_test_db().await;

        // 创建用户
        let user_id = create_test_user(&pool, "author").await;

        // 创建标签
        let tag1_id = insert_tag(&pool, "Rust", "rust").await.unwrap();
        let tag2_id = insert_tag(&pool, "Python", "python").await.unwrap();
        let _tag3_id = insert_tag(&pool, "Empty", "empty").await.unwrap(); // 无文章

        // 创建文章
        let post1_id = crate::modules::post::repository::insert_post(
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
                scheduled_at: None,
            },
        )
        .await
        .unwrap();

        let post2_id = crate::modules::post::repository::insert_post(
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
                scheduled_at: None,
            },
        )
        .await
        .unwrap();

        // 关联标签
        crate::modules::post::repository::replace_tags(&pool, &post1_id, &[tag1_id.clone()])
            .await
            .unwrap();
        crate::modules::post::repository::replace_tags(
            &pool,
            &post2_id,
            &[tag1_id.clone(), tag2_id.clone()],
        )
        .await
        .unwrap();

        // 查询标签云
        let tags = get_all_tags_with_count(&pool).await.unwrap();

        // 应该只返回 rust 和 python，不包含 empty
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].slug, "rust"); // 2 篇文章，排第一
        assert_eq!(tags[0].post_count, 2);
        assert_eq!(tags[1].slug, "python"); // 1 篇文章
        assert_eq!(tags[1].post_count, 1);
    }

    #[tokio::test]
    async fn get_all_tags_with_count_excludes_draft_posts() {
        let pool = setup_test_db().await;

        let user_id = create_test_user(&pool, "author").await;

        let tag_id = insert_tag(&pool, "Draft", "draft").await.unwrap();

        // 创建草稿文章
        let post_id = crate::modules::post::repository::insert_post(
            &pool,
            crate::modules::post::post_types::NewPostParams {
                author_id: &user_id,
                title: "Draft Post",
                slug: "draft-post",
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
                scheduled_at: None,
            },
        )
        .await
        .unwrap();

        crate::modules::post::repository::replace_tags(&pool, &post_id, &[tag_id])
            .await
            .unwrap();

        let tags = get_all_tags_with_count(&pool).await.unwrap();
        assert_eq!(tags.len(), 0); // 草稿不计入
    }
}
