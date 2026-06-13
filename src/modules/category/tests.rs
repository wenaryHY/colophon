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

    #[tokio::test]
    async fn get_by_slug_returns_category_when_exists() {
        let pool = setup_test_db().await;

        let cat_id = insert_category(&pool, "Technology", "technology", None, None, 0)
            .await
            .unwrap();

        let result = get_by_slug(&pool, "technology").await.unwrap();
        assert!(result.is_some());
        let cat = result.unwrap();
        assert_eq!(cat.id, cat_id);
        assert_eq!(cat.name, "Technology");
        assert_eq!(cat.slug, "technology");
    }

    #[tokio::test]
    async fn get_by_slug_returns_none_when_not_exists() {
        let pool = setup_test_db().await;

        let result = get_by_slug(&pool, "nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn get_by_slug_ignores_deleted_categories() {
        let pool = setup_test_db().await;

        let cat_id = insert_category(&pool, "Deleted", "deleted", None, None, 0)
            .await
            .unwrap();
        delete_category(&pool, &cat_id).await.unwrap();

        let result = get_by_slug(&pool, "deleted").await.unwrap();
        assert!(result.is_none());
    }
}
