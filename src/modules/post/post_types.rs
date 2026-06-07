//! 文章状态、可见性、内容类型的零成本抽象。
//!
//! ## 设计原则
//! - 编译期类型安全：枚举替代字符串，编译期检查合法性
//! - 零开销：Copy + field-less enum，编译器 niche 优化
//! - API 兼容：serde 序列化为 snake_case 字符串，与原有 JSON 格式一致
//! - sqlx 兼容：实现 Type/Decode/Encode，可直接用于 FromRow 和参数绑定

use std::str::FromStr;

use crate::shared::error::AppError;

// ── PostStatus ──────────────────────────────────────────────────────────

/// 文章发布状态。
///
/// ## 序列化
/// `#[serde(rename_all = "snake_case")]` 保证 JSON 格式为 `"draft"` / `"published"` / `"trashed"`，
/// 与原有 API 契约完全兼容。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PostStatus {
    Draft,
    Published,
    Trashed,
}

impl Default for PostStatus {
    fn default() -> Self {
        Self::Draft
    }
}

impl PostStatus {
    /// 返回数据库存储字符串，零分配。
    #[inline(always)]
    pub fn as_str(self) -> &'static str {
        match self {
            PostStatus::Draft => "draft",
            PostStatus::Published => "published",
            PostStatus::Trashed => "trashed",
        }
    }
}

impl std::fmt::Display for PostStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for PostStatus {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "draft" => Ok(PostStatus::Draft),
            "published" => Ok(PostStatus::Published),
            "trashed" => Ok(PostStatus::Trashed),
            other => Err(AppError::BadRequest(format!(
                "invalid post status: '{other}'"
            ))),
        }
    }
}

// ── Visibility ──────────────────────────────────────────────────────────

/// 文章可见性。
///
/// ## 序列化
/// JSON 格式为 `"public"` / `"private"`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    Public,
    Private,
}

impl Visibility {
    /// 返回数据库存储字符串，零分配。
    #[inline(always)]
    pub fn as_str(self) -> &'static str {
        match self {
            Visibility::Public => "public",
            Visibility::Private => "private",
        }
    }
}

impl std::fmt::Display for Visibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Visibility {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "public" => Ok(Visibility::Public),
            "private" => Ok(Visibility::Private),
            other => Err(AppError::BadRequest(format!(
                "invalid visibility: '{other}'"
            ))),
        }
    }
}

// ── ContentType ─────────────────────────────────────────────────────────

/// 内容类型。
///
/// ## 序列化
/// JSON 格式为 `"post"` / `"page"`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentType {
    Post,
    Page,
}

impl Default for ContentType {
    fn default() -> Self {
        Self::Post
    }
}

impl ContentType {
    /// 返回数据库存储字符串，零分配。
    #[inline(always)]
    pub fn as_str(self) -> &'static str {
        match self {
            ContentType::Post => "post",
            ContentType::Page => "page",
        }
    }

    /// 是否为 page 类型。
    #[inline(always)]
    pub fn is_page(self) -> bool {
        matches!(self, ContentType::Page)
    }

    /// 是否为 post 类型。
    #[inline(always)]
    pub fn is_post(self) -> bool {
        matches!(self, ContentType::Post)
    }
}

impl std::fmt::Display for ContentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ContentType {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "post" => Ok(ContentType::Post),
            "page" => Ok(ContentType::Page),
            other => Err(AppError::BadRequest(format!(
                "invalid content_type: '{other}', must be 'post' or 'page'"
            ))),
        }
    }
}

// ── sqlx 实现 ───────────────────────────────────────────────────────────
// 将枚举作为 String 存储到 SQLite，从 String 读取解析。
// 模式与 src/shared/role.rs 中 Role 的 sqlx 实现一致。

macro_rules! impl_sqlx_for_str_enum {
    ($ty:ty) => {
        impl sqlx::Type<sqlx::Sqlite> for $ty {
            fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
                <String as sqlx::Type<sqlx::Sqlite>>::type_info()
            }
        }

        impl<'r> sqlx::Decode<'r, sqlx::Sqlite> for $ty {
            fn decode(
                value: sqlx::sqlite::SqliteValueRef<'r>,
            ) -> Result<Self, sqlx::error::BoxDynError> {
                let s = <String as sqlx::Decode<sqlx::Sqlite>>::decode(value)?;
                s.parse().map_err(Into::into)
            }
        }

        impl<'q> sqlx::Encode<'q, sqlx::Sqlite> for $ty {
            fn encode_by_ref(
                &self,
                args: &mut Vec<sqlx::sqlite::SqliteArgumentValue<'q>>,
            ) -> sqlx::encode::IsNull {
                <String as sqlx::Encode<sqlx::Sqlite>>::encode(self.to_string(), args)
            }
        }
    };
}

impl_sqlx_for_str_enum!(PostStatus);
impl_sqlx_for_str_enum!(Visibility);
impl_sqlx_for_str_enum!(ContentType);

// ── 参数 struct：替代 16/17 个零散参数的巨型函数签名 ────────────

/// `insert_post` 的参数集合。
/// 替代原先 16 个零散参数，编译器保证调用处不遗漏字段。
pub struct NewPostParams<'a> {
    pub author_id: &'a str,
    pub title: &'a str,
    pub slug: &'a str,
    pub excerpt: Option<&'a str>,
    pub content_md: &'a str,
    pub content_html: &'a str,
    pub cover_media_id: Option<&'a str>,
    pub status: PostStatus,
    pub visibility: Visibility,
    pub category_id: Option<&'a str>,
    pub allow_comment: bool,
    pub pinned: bool,
    pub content_type: ContentType,
    pub custom_html_path: Option<&'a str>,
    pub page_render_mode: &'a str,
}

/// `update_post` 的参数集合。
pub struct UpdatePostParams<'a> {
    pub post_id: &'a str,
    pub title: &'a str,
    pub slug: &'a str,
    pub excerpt: Option<&'a str>,
    pub content_md: &'a str,
    pub content_html: &'a str,
    pub cover_media_id: Option<&'a str>,
    pub status: PostStatus,
    pub visibility: Visibility,
    pub category_id: Option<&'a str>,
    pub allow_comment: bool,
    pub pinned: bool,
    pub content_type: ContentType,
    pub custom_html_path: Option<&'a str>,
    pub page_render_mode: &'a str,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── PostStatus ──

    #[test]
    fn post_status_as_str() {
        assert_eq!(PostStatus::Draft.as_str(), "draft");
        assert_eq!(PostStatus::Published.as_str(), "published");
        assert_eq!(PostStatus::Trashed.as_str(), "trashed");
    }

    #[test]
    fn post_status_display_matches_as_str() {
        assert_eq!(PostStatus::Draft.to_string(), "draft");
        assert_eq!(PostStatus::Published.to_string(), "published");
    }

    #[test]
    fn post_status_from_str_valid() {
        assert_eq!("draft".parse::<PostStatus>().unwrap(), PostStatus::Draft);
        assert_eq!("published".parse::<PostStatus>().unwrap(), PostStatus::Published);
        assert_eq!("trashed".parse::<PostStatus>().unwrap(), PostStatus::Trashed);
    }

    #[test]
    fn post_status_from_str_invalid() {
        assert!("archived".parse::<PostStatus>().is_err());
        assert!("".parse::<PostStatus>().is_err());
    }

    #[test]
    fn post_status_default_is_draft() {
        assert_eq!(PostStatus::default(), PostStatus::Draft);
    }

    #[test]
    fn post_status_serde_roundtrip() {
        let json = serde_json::to_string(&PostStatus::Published).unwrap();
        assert_eq!(json, "\"published\"");
        let parsed: PostStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, PostStatus::Published);
    }

    #[test]
    fn post_status_serde_rejects_unknown() {
        assert!(serde_json::from_str::<PostStatus>("\"archived\"").is_err());
    }

    #[test]
    fn post_status_is_copy() {
        let s = PostStatus::Published;
        let copy = s;
        assert_eq!(s, copy);
    }

    // ── Visibility ──

    #[test]
    fn visibility_as_str() {
        assert_eq!(Visibility::Public.as_str(), "public");
        assert_eq!(Visibility::Private.as_str(), "private");
    }

    #[test]
    fn visibility_display_matches_as_str() {
        assert_eq!(Visibility::Public.to_string(), "public");
        assert_eq!(Visibility::Private.to_string(), "private");
    }

    #[test]
    fn visibility_from_str_valid() {
        assert_eq!("public".parse::<Visibility>().unwrap(), Visibility::Public);
        assert_eq!("private".parse::<Visibility>().unwrap(), Visibility::Private);
    }

    #[test]
    fn visibility_from_str_invalid() {
        assert!("secret".parse::<Visibility>().is_err());
        assert!("".parse::<Visibility>().is_err());
    }

    #[test]
    fn visibility_serde_roundtrip() {
        let json = serde_json::to_string(&Visibility::Public).unwrap();
        assert_eq!(json, "\"public\"");
        let parsed: Visibility = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, Visibility::Public);
    }

    #[test]
    fn visibility_is_copy() {
        let v = Visibility::Private;
        let copy = v;
        assert_eq!(v, copy);
    }

    // ── ContentType ──

    #[test]
    fn content_type_as_str() {
        assert_eq!(ContentType::Post.as_str(), "post");
        assert_eq!(ContentType::Page.as_str(), "page");
    }

    #[test]
    fn content_type_is_page() {
        assert!(ContentType::Page.is_page());
        assert!(!ContentType::Post.is_page());
    }

    #[test]
    fn content_type_is_post() {
        assert!(ContentType::Post.is_post());
        assert!(!ContentType::Page.is_post());
    }

    #[test]
    fn content_type_display_matches_as_str() {
        assert_eq!(ContentType::Post.to_string(), "post");
        assert_eq!(ContentType::Page.to_string(), "page");
    }

    #[test]
    fn content_type_from_str_valid() {
        assert_eq!("post".parse::<ContentType>().unwrap(), ContentType::Post);
        assert_eq!("page".parse::<ContentType>().unwrap(), ContentType::Page);
    }

    #[test]
    fn content_type_from_str_invalid() {
        assert!("article".parse::<ContentType>().is_err());
        assert!("".parse::<ContentType>().is_err());
    }

    #[test]
    fn content_type_default_is_post() {
        assert_eq!(ContentType::default(), ContentType::Post);
    }

    #[test]
    fn content_type_serde_roundtrip() {
        let json = serde_json::to_string(&ContentType::Page).unwrap();
        assert_eq!(json, "\"page\"");
        let parsed: ContentType = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, ContentType::Page);
    }

    #[test]
    fn content_type_serde_rejects_unknown() {
        assert!(serde_json::from_str::<ContentType>("\"article\"").is_err());
    }

    #[test]
    fn content_type_is_copy() {
        let ct = ContentType::Page;
        let copy = ct;
        assert_eq!(ct, copy);
    }
}
