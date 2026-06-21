//! OpenAPI 文档定义与 Swagger UI 挂载入口。
//!
//! ## 全局 JWT Security Scheme
//! 通过 `#[openapi(security(("jwt" = [])))]` 为整个文档声明默认安全策略。
//! 在需要认证的接口上重复声明 `security(("jwt" = []))` 即可关联。
//!
//! ## 泛型响应类型处理
//! `ApiResponse<T>` 与 `PaginatedResponse<T>` 是泛型。utoipa 5 移除了 `#[aliases]`，
//! 但 `schemas()` 支持注册泛型具体实例（如 `ApiResponse<LoginResponseData>`），
//! 编译期会为每个具体类型生成独立的具名 schema。

use utoipa::{
    openapi::security::{ApiKey, ApiKeyValue, SecurityScheme},
    Modify, OpenApi,
};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Colophon CMS API",
        version = "1.0.0",
        description = "个人博客引擎 / 无头 CMS API 文档",
        license(
            name = "AGPL-3.0",
            url = "https://www.gnu.org/licenses/agpl-3.0.html"
        )
    ),
    paths(
        crate::bootstrap::router::health_check,
        crate::modules::auth::handler::login,
        crate::modules::user::handler::me,
        crate::modules::post::handler::list_admin_posts,
        crate::modules::post::handler::create_post,
    ),
    components(
        schemas(
            crate::bootstrap::router::HealthResponse,
            crate::modules::auth::dto::LoginRequest,
            crate::modules::auth::dto::LoginResponseData,
            crate::modules::auth::dto::AuthUserInfo,
            crate::modules::user::domain::CurrentUser,
            crate::modules::post::dto::CreatePostRequest,
            crate::modules::post::dto::AdminPostResponse,
            crate::modules::post::domain::AdminPost,
            crate::modules::tag::domain::Tag,
            crate::shared::role::Role,
            crate::modules::post::post_types::PostStatus,
            crate::modules::post::post_types::ContentType,
            crate::modules::post::post_types::Visibility,
            crate::shared::response::PaginationMeta,
            crate::shared::response::ApiResponse<crate::bootstrap::router::HealthResponse>,
            crate::shared::response::ApiResponse<crate::modules::auth::dto::LoginResponseData>,
            crate::shared::response::ApiResponse<crate::modules::user::domain::CurrentUser>,
            crate::shared::response::ApiResponse<crate::modules::post::dto::AdminPostResponse>,
            crate::shared::response::PaginatedResponse<crate::modules::post::dto::AdminPostResponse>,
            crate::shared::response::ApiResponse<crate::shared::response::PaginatedResponse<crate::modules::post::dto::AdminPostResponse>>,
        )
    ),
    modifiers(&JwtSecurityModifier),
    security(("jwt" = []))
)]
pub struct ApiDoc;

/// 为 OpenAPI 文档注入全局 JWT Security Scheme。
///
/// `security(("jwt" = []))` 仅声明某个接口需要名为 `jwt` 的安全策略，
/// 策略本身的具体定义（HTTP header 名、认证方式）由本结构体的 `Modify` 实现提供。
struct JwtSecurityModifier;

impl Modify for JwtSecurityModifier {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "jwt",
                SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::with_description(
                    "Authorization",
                    "JWT access token。格式：`Bearer <token>`。登录接口返回，或从 session cookie 提取。",
                ))),
            );
        }
    }
}
