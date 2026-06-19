use std::str::FromStr;

use uuid::Uuid;

use crate::modules::{
    post::{
        post_types::{ContentType, NewPostParams, PostStatus, Visibility},
        repository as post_repository,
    },
    setting::repository as setting_repository,
    setup::domain::SetupStage,
};

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

    insert_default_cookie_policy_page(&mut tx, &user_id).await?;

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

/// 系统初始化时自动创建默认的 Cookie 政策页面。
/// 幂等：如果 slug 已存在则跳过，重复运行 setup 不会出错。
pub(crate) async fn insert_default_cookie_policy_page(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    author_id: &str,
) -> Result<(), sqlx::Error> {
    let already_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM posts WHERE slug = 'cookie-policy')")
            .fetch_one(&mut **tx)
            .await?;

    if already_exists {
        return Ok(());
    }

    let content_md = r#"## Cookie 政策

本网站仅使用两个第一方必要 Cookie，不会追踪您的浏览行为，也不会与任何第三方分享数据。

### 我们使用的 Cookie

| Cookie 名称 | 用途 | 有效期 |
| --- | --- | --- |
| `colophon_session` | 登录认证 | 会话期间（关闭浏览器后自动删除） |
| `lang` | 记住您的语言偏好 | 90 天 |

#### `colophon_session`

用于识别已登录用户的身份凭证。仅在您主动登录后设置，关闭浏览器后自动失效。

#### `lang`

用于记住您通过语言切换器选择的界面语言，以便下次访问时自动显示您偏好的语言版本。

### 我们不使用以下内容

- 第三方 Cookie
- 分析 / 统计追踪脚本
- 广告追踪器
- 社交媒体追踪器

### 管理 Cookie

您可以在浏览器设置中随时查看、删除或阻止 Cookie。大多数浏览器的设置路径为：设置 → 隐私与安全 → Cookie 和站点数据。

请注意，禁用必要 Cookie 可能导致登录功能无法正常使用。

### 数据所有权

本网站运行在您自己的服务器上，所有数据由您自己掌控。我们不会将任何数据发送到外部服务。

---

## Cookie Policy

This site uses only two first-party essential cookies. We do not track your browsing behavior or share any data with third parties.

### Cookies We Use

| Cookie Name | Purpose | Duration |
| --- | --- | --- |
| `colophon_session` | Authentication | Session (deleted when you close your browser) |
| `lang` | Language preference | 90 days |

#### `colophon_session`

Identifies logged-in users. Only set after you actively sign in. Automatically expires when you close your browser.

#### `lang`

Remembers your preferred interface language selected via the language switcher, so you see your preferred language on your next visit.

### What We Don't Use

- Third-party cookies
- Analytics or tracking scripts
- Advertising trackers
- Social media widgets

### Managing Cookies

You can view, delete, or block cookies at any time in your browser settings. Most browsers provide this under: Settings → Privacy & Security → Cookies and site data.

Please note that disabling essential cookies may prevent login functionality from working.

### Data Ownership

This site runs on your own server, and all data remains under your control. We do not send any data to external services.
"#;

    let content_html = r#"<h2 id="cookie-policy-cn">Cookie 政策</h2>
<p>本网站仅使用两个第一方必要 Cookie，不会追踪您的浏览行为，也不会与任何第三方分享数据。</p>
<h3 id="cookies-we-use-cn">我们使用的 Cookie</h3>
<table>
<thead>
<tr>
<th>Cookie 名称</th>
<th>用途</th>
<th>有效期</th>
</tr>
</thead>
<tbody>
<tr>
<td><code>colophon_session</code></td>
<td>登录认证</td>
<td>会话期间（关闭浏览器后自动删除）</td>
</tr>
<tr>
<td><code>lang</code></td>
<td>记住您的语言偏好</td>
<td>90 天</td>
</tr>
</tbody>
</table>
<h4 id="colophon-session-cn"><code>colophon_session</code></h4>
<p>用于识别已登录用户的身份凭证。仅在您主动登录后设置，关闭浏览器后自动失效。</p>
<h4 id="lang-cn"><code>lang</code></h4>
<p>用于记住您通过语言切换器选择的界面语言，以便下次访问时自动显示您偏好的语言版本。</p>
<h3 id="what-we-dont-use-cn">我们不使用以下内容</h3>
<ul>
<li>第三方 Cookie</li>
<li>分析 / 统计追踪脚本</li>
<li>广告追踪器</li>
<li>社交媒体追踪器</li>
</ul>
<h3 id="managing-cookies-cn">管理 Cookie</h3>
<p>您可以在浏览器设置中随时查看、删除或阻止 Cookie。大多数浏览器的设置路径为：设置 → 隐私与安全 → Cookie 和站点数据。</p>
<p>请注意，禁用必要 Cookie 可能导致登录功能无法正常使用。</p>
<h3 id="data-ownership-cn">数据所有权</h3>
<p>本网站运行在您自己的服务器上，所有数据由您自己掌控。我们不会将任何数据发送到外部服务。</p>
<hr>
<h2 id="cookie-policy-en">Cookie Policy</h2>
<p>This site uses only two first-party essential cookies. We do not track your browsing behavior or share any data with third parties.</p>
<h3 id="cookies-we-use-en">Cookies We Use</h3>
<table>
<thead>
<tr>
<th>Cookie Name</th>
<th>Purpose</th>
<th>Duration</th>
</tr>
</thead>
<tbody>
<tr>
<td><code>colophon_session</code></td>
<td>Authentication</td>
<td>Session (deleted when you close your browser)</td>
</tr>
<tr>
<td><code>lang</code></td>
<td>Language preference</td>
<td>90 days</td>
</tr>
</tbody>
</table>
<h4 id="colophon-session-en"><code>colophon_session</code></h4>
<p>Identifies logged-in users. Only set after you actively sign in. Automatically expires when you close your browser.</p>
<h4 id="lang-en"><code>lang</code></h4>
<p>Remembers your preferred interface language selected via the language switcher, so you see your preferred language on your next visit.</p>
<h3 id="what-we-dont-use-en">What We Don't Use</h3>
<ul>
<li>Third-party cookies</li>
<li>Analytics or tracking scripts</li>
<li>Advertising trackers</li>
<li>Social media widgets</li>
</ul>
<h3 id="managing-cookies-en">Managing Cookies</h3>
<p>You can view, delete, or block cookies at any time in your browser settings. Most browsers provide this under: Settings → Privacy &amp; Security → Cookies and site data.</p>
<p>Please note that disabling essential cookies may prevent login functionality from working.</p>
<h3 id="data-ownership-en">Data Ownership</h3>
<p>This site runs on your own server, and all data remains under your control. We do not send any data to external services.</p>
"#;

    let params = NewPostParams {
        author_id,
        title: "Cookie 政策",
        slug: "cookie-policy",
        excerpt: None,
        content_md,
        content_html,
        cover_media_id: None,
        status: PostStatus::Published,
        visibility: Visibility::Public,
        category_id: None,
        allow_comment: false,
        pinned: false,
        content_type: ContentType::Page,
        custom_html_path: None,
        page_render_mode: "editor",
    };

    post_repository::insert_post(&mut **tx, params).await?;
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

#[cfg(test)]
mod tests {
    use super::insert_default_cookie_policy_page;
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

    /// 在所有测试中复用同一个测试用户插入逻辑，避免重复代码。
    async fn insert_test_user(pool: &sqlx::SqlitePool, user_id: &str) {
        sqlx::query(
            "INSERT INTO users (id, username, email, password_hash, display_name, role, status)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(user_id)
        .bind("testuser")
        .bind("test@example.com")
        .bind("hash")
        .bind("Test User")
        .bind("admin")
        .bind("active")
        .execute(pool)
        .await
        .expect("insert test user");
    }

    #[tokio::test]
    async fn test_insert_cookie_policy_page_creates_record() {
        let pool = setup_test_db().await;
        let user_id = "test-user-id";
        insert_test_user(&pool, user_id).await;

        let mut tx = pool.begin().await.expect("begin tx");
        insert_default_cookie_policy_page(&mut tx, user_id)
            .await
            .expect("insert cookie policy page");
        tx.commit().await.expect("commit");

        let (slug, content_type, status): (String, String, String) = sqlx::query_as(
            "SELECT slug, content_type, status FROM posts WHERE slug = 'cookie-policy'",
        )
        .fetch_one(&pool)
        .await
        .expect("fetch page");

        assert_eq!(slug, "cookie-policy");
        assert_eq!(content_type, "page");
        assert_eq!(status, "published");
    }

    #[tokio::test]
    async fn test_insert_cookie_policy_page_is_idempotent() {
        let pool = setup_test_db().await;
        let user_id = "test-user-id";
        insert_test_user(&pool, user_id).await;

        // 第一次插入
        let mut tx1 = pool.begin().await.expect("begin tx1");
        insert_default_cookie_policy_page(&mut tx1, user_id)
            .await
            .expect("first insert");
        tx1.commit().await.expect("commit");

        // 第二次插入（幂等：应跳过，不报错）
        let mut tx2 = pool.begin().await.expect("begin tx2");
        insert_default_cookie_policy_page(&mut tx2, user_id)
            .await
            .expect("second insert should not error");
        tx2.commit().await.expect("commit");

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM posts WHERE slug = 'cookie-policy'",
        )
        .fetch_one(&pool)
        .await
        .expect("count");

        assert_eq!(count, 1, "should have exactly one cookie-policy page");
    }

    #[tokio::test]
    async fn test_insert_cookie_policy_page_has_content() {
        let pool = setup_test_db().await;
        let user_id = "test-user-id";
        insert_test_user(&pool, user_id).await;

        let mut tx = pool.begin().await.expect("begin tx");
        insert_default_cookie_policy_page(&mut tx, user_id)
            .await
            .expect("insert");
        tx.commit().await.expect("commit");

        let content: String = sqlx::query_scalar(
            "SELECT content_html FROM posts WHERE slug = 'cookie-policy'",
        )
        .fetch_one(&pool)
        .await
        .expect("fetch content");

        assert!(!content.is_empty(), "cookie policy page should have content");
        // 验证包含关键中文内容
        assert!(content.contains("Cookie"), "should mention Cookie");
    }
}
