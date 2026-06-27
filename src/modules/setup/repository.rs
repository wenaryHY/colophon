use std::str::FromStr;

use uuid::Uuid;

use crate::modules::{setting::repository as setting_repository, setup::domain::SetupStage};

pub struct SetupSnapshot {
    pub stage: SetupStage,
    pub persisted_stage: Option<SetupStage>,
    pub setup_completed: bool,
    pub user_count: i64,
    pub site_title: String,
    pub site_description: String,
    pub site_url: String,
    pub admin_url: String,
    pub allow_register: bool,
}

impl SetupSnapshot {
    pub fn needs_state_backfill(&self) -> bool {
        self.persisted_stage != Some(self.stage)
            || self.setup_completed != self.stage.is_completed()
    }
}

pub struct SetupWriteModel {
    pub site_title: String,
    pub site_description: String,
    pub site_url: String,
    pub admin_url: String,
    pub allow_register: bool,
    pub username: String,
    pub email: String,
    pub display_name: String,
    pub password_hash: String,
}

pub async fn load_snapshot<'e, E>(executor: E) -> Result<SetupSnapshot, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite> + Copy,
{
    let setup_completed = setting_repository::get_bool(executor, "setup_completed", false).await?;
    let user_count = user_count(executor).await?;
    let persisted_stage = setting_repository::get_optional_string(executor, "setup_stage")
        .await?
        .and_then(|value| SetupStage::from_str(&value).ok());
    let stage =
        persisted_stage.unwrap_or_else(|| SetupStage::infer_legacy(setup_completed, user_count));

    Ok(SetupSnapshot {
        stage,
        persisted_stage,
        setup_completed,
        user_count,
        site_title: setting_repository::get_string(executor, "site_title", "Colophon").await?,
        site_description: setting_repository::get_string(executor, "site_description", "").await?,
        site_url: setting_repository::get_string(executor, "site_url", "").await?,
        admin_url: setting_repository::get_string(executor, "admin_url", "").await?,
        allow_register: setting_repository::get_bool(executor, "allow_register", true).await?,
    })
}

pub async fn persist_stage<'e, E>(executor: E, stage: SetupStage) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite> + Copy,
{
    setting_repository::upsert(executor, "setup_stage", stage.as_str()).await?;
    setting_repository::upsert(
        executor,
        "setup_completed",
        if stage.is_completed() {
            "true"
        } else {
            "false"
        },
    )
    .await
}

pub async fn create_installation(
    pool: &sqlx::SqlitePool,
    model: &SetupWriteModel,
) -> Result<String, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let user_id = Uuid::new_v4().to_string();

    sqlx::query(
        "INSERT INTO users (
            id, username, email, password_hash, display_name, role, status, theme_preference
        ) VALUES (?, ?, ?, ?, ?, 'admin', 'active', 'system')",
    )
    .bind(&user_id)
    .bind(&model.username)
    .bind(&model.email)
    .bind(&model.password_hash)
    .bind(&model.display_name)
    .execute(&mut *tx)
    .await?;

    upsert_setting(&mut *tx, "site_title", &model.site_title).await?;
    upsert_setting(&mut *tx, "site_description", &model.site_description).await?;
    upsert_setting(&mut *tx, "site_url", &model.site_url).await?;
    upsert_setting(&mut *tx, "admin_url", &model.admin_url).await?;
    upsert_setting(
        &mut *tx,
        "allow_register",
        if model.allow_register {
            "true"
        } else {
            "false"
        },
    )
    .await?;
    upsert_setting(&mut *tx, "setup_stage", SetupStage::Completed.as_str()).await?;
    upsert_setting(&mut *tx, "setup_completed", "true").await?;

    tx.commit().await?;
    Ok(user_id)
}

async fn upsert_setting<'e, E>(executor: E, key: &str, value: &str) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query(
        "INSERT INTO settings (key, value, updated_at)
         VALUES (?, ?, datetime('now'))
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')",
    )
    .bind(key)
    .bind(value)
    .execute(executor)
    .await?;
    Ok(())
}

async fn user_count<'e, E>(executor: E) -> Result<i64, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(executor)
        .await
}
