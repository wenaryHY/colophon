/// 生成标准 CRUD handler 函数。
///
/// 用法:
/// ```ignore
/// crud_handlers! {
///     module = super,
///     entity = Category,
///     create_req = CreateCategoryRequest,
///     update_req = UpdateCategoryRequest,
///     list_fn = list_categories,
///     create_fn = create_category,
///     update_fn = update_category,
///     delete_fn = delete_category,
/// }
/// ```
///
/// `module` 参数用于指定 service 模块的相对路径，通常为 `super`
/// （即 handler 所在模块的父模块，其中包含 `service` 子模块）。
#[macro_export]
macro_rules! crud_handlers {
    (
        module = $module:tt,
        entity = $entity:ty,
        create_req = $create_req:ty,
        update_req = $update_req:ty,
        list_fn = $list_fn:ident,
        create_fn = $create_fn:ident,
        update_fn = $update_fn:ident,
        delete_fn = $delete_fn:ident $(,)?
    ) => {
        #[allow(clippy::too_many_arguments)]
        pub async fn $list_fn(
            axum::extract::State(state): axum::extract::State<std::sync::Arc<$crate::state::AppState>>,
        ) -> $crate::shared::error::AppResult<
            axum::Json<$crate::shared::response::ApiResponse<Vec<$entity>>>,
        > {
            let data = $module::service::$list_fn(state).await?;
            Ok(axum::Json($crate::shared::response::ApiResponse::success(data)))
        }

        #[allow(clippy::too_many_arguments)]
        pub async fn $create_fn(
            axum::extract::State(state): axum::extract::State<std::sync::Arc<$crate::state::AppState>>,
            _admin: $crate::shared::auth::AdminUser,
            axum::Json(body): axum::Json<$create_req>,
        ) -> $crate::shared::error::AppResult<
            axum::Json<$crate::shared::response::ApiResponse<$entity>>,
        > {
            let data = $module::service::$create_fn(state, body).await?;
            Ok(axum::Json($crate::shared::response::ApiResponse::success(data)))
        }

        #[allow(clippy::too_many_arguments)]
        pub async fn $update_fn(
            axum::extract::State(state): axum::extract::State<std::sync::Arc<$crate::state::AppState>>,
            _admin: $crate::shared::auth::AdminUser,
            axum::extract::Path(id): axum::extract::Path<String>,
            axum::Json(body): axum::Json<$update_req>,
        ) -> $crate::shared::error::AppResult<
            axum::Json<$crate::shared::response::ApiResponse<$entity>>,
        > {
            let data = $module::service::$update_fn(state, &id, body).await?;
            Ok(axum::Json($crate::shared::response::ApiResponse::success(data)))
        }

        #[allow(clippy::too_many_arguments)]
        pub async fn $delete_fn(
            axum::extract::State(state): axum::extract::State<std::sync::Arc<$crate::state::AppState>>,
            _admin: $crate::shared::auth::AdminUser,
            axum::extract::Path(id): axum::extract::Path<String>,
        ) -> $crate::shared::error::AppResult<
            axum::Json<$crate::shared::response::ApiResponse<serde_json::Value>>,
        > {
            let data = $module::service::$delete_fn(state, &id).await?;
            Ok(axum::Json($crate::shared::response::ApiResponse::success(data)))
        }
    };
}
