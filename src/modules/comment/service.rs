use std::sync::Arc;

use crate::{
    modules::{post::repository as post_repository, setting::repository as setting_repository},
    shared::{
        auth::AuthUser,
        error::{AppError, AppResult},
        pagination::PaginationQuery,
        response::PaginatedResponse,
    },
    state::AppState,
    ws::ServerEvent,
};

use crate::modules::plugin::hook::{HookContext, HookData, CommentBeforeCreateData};

use super::{
    domain::{AdminCommentItem, CommentItem},
    dto::{CommentQuery, CreateCommentRequest},
    repository,
};

fn moderation_status(mode: &str, has_approved_comment: bool) -> &'static str {
    match mode {
        "none" => "approved",
        "first_comment" if has_approved_comment => "approved",
        _ => "pending",
    }
}

pub async fn list_post_comments(state: Arc<AppState>, slug: &str) -> AppResult<Vec<CommentItem>> {
    tracing::debug!(
        module = "comment",
        event = "list_post_comments",
        slug = %slug,
        "listing approved post comments"
    );
    let post = post_repository::find_comment_target(&state.pool, slug)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(repository::list_approved_for_post(&state.pool, &post.id).await?)
}

pub async fn create_comment(
    state: Arc<AppState>,
    auth: &AuthUser,
    slug: &str,
    body: CreateCommentRequest,
) -> AppResult<serde_json::Value> {
    if body.content.trim().is_empty() {
        tracing::warn!(
            module = "comment",
            event = "create_rejected_empty",
            user_id = %auth.id,
            slug = %slug,
            "comment creation rejected"
        );
        return Err(AppError::BadRequest("comment content is required".into()));
    }

    let allow_comment = setting_repository::get_bool(&state.pool, "allow_comment", true).await?;
    // 注意：handler 层已通过 AuthUser extractor 强制要求登录，
    // 因此 comment_require_login setting 语义上已是"允许游客评论"（当前 AuthUser 已隐含登录），
    // 此处只保留 allow_comment 总开关检查。
    if !allow_comment {
        tracing::warn!(
            module = "comment",
            event = "create_rejected_disabled",
            user_id = %auth.id,
            slug = %slug,
            "comment creation rejected"
        );
        return Err(AppError::Forbidden);
    }

    let max_length =
        setting_repository::get_string(&state.pool, "comment_max_length", "2000").await?;
    let max_length = max_length.parse::<usize>().unwrap_or(2000);
    if body.content.chars().count() > max_length {
        tracing::warn!(
            module = "comment",
            event = "create_rejected_too_long",
            user_id = %auth.id,
            slug = %slug,
            content_length = body.content.chars().count(),
            max_length = max_length,
            "comment creation rejected"
        );
        return Err(AppError::BadRequest("comment is too long".into()));
    }

    let post = post_repository::find_comment_target(&state.pool, slug)
        .await?
        .ok_or(AppError::NotFound)?;
    if post.status != "published" || post.visibility != "public" || post.allow_comment == 0 {
        tracing::warn!(
            module = "comment",
            event = "create_rejected_post_state",
            user_id = %auth.id,
            slug = %slug,
            post_status = %post.status,
            post_visibility = %post.visibility,
            post_allow_comment = post.allow_comment,
            "comment creation rejected"
        );
        return Err(AppError::Forbidden);
    }

    let moderation_mode =
        setting_repository::get_string(&state.pool, "comment_moderation_mode", "all").await?;
    let has_approved = repository::count_approved_by_user(&state.pool, &auth.id).await? > 0;
    let status = moderation_status(&moderation_mode, has_approved);

    tracing::info!(
        module = "comment",
        event = "create_attempt",
        user_id = %auth.id,
        username = %auth.username,
        slug = %slug,
        moderation_mode = %moderation_mode,
        has_approved_comment = has_approved,
        final_status = %status,
        "creating comment"
    );

    // comment.before_create filter hook：允许插件在写 DB 前修改评论内容
    let mut content = body.content.trim().to_string();
    let hook_registry = state.plugin_manager.hook_registry();
    let mut comment_ctx = HookContext {
        hook_name: "comment.before_create".into(),
        data: HookData::CommentBeforeCreate(CommentBeforeCreateData {
            content: content.clone(),
            author_name: auth.username.clone(),
            author_email: None,
            post_id: post.id.clone(),
            post_title: post.title.clone(),
            request_ip: None,
        }),
    };
    hook_registry
        .dispatch_filter("comment.before_create", &mut comment_ctx)
        .await?;
    // 从 comment_ctx.data 中提取可能被 Filter 修改的 content
    if let HookData::CommentBeforeCreate(modified) = comment_ctx.data {
        content = modified.content;
    }

    let (comment_id, created_at) = repository::insert_comment(
        &state.pool,
        &post.id,
        &auth.id,
        &content,
        body.parent_id.as_deref(),
        status,
    )
    .await?;

    // 通过 WebSocket 广播评论创建事件（管理后台实时刷新）
    let event = ServerEvent::CommentCreated {
        id: comment_id.clone(),
        post_id: post.id.clone(),
        author_name: auth.username.clone(),
        content: content.clone(),
        status: status.to_string(),
        created_at,
    };
    let _ = state.event_tx.send(event);

    tracing::info!(
        module = "comment",
        event = "create_success",
        comment_id = %comment_id,
        post_id = %post.id,
        user_id = %auth.id,
        status = %status,
        "comment created"
    );

    Ok(serde_json::json!({
        "id": comment_id,
        "status": status,
    }))
}

pub async fn my_comments(
    state: Arc<AppState>,
    auth: &AuthUser,
    query: CommentQuery,
) -> AppResult<PaginatedResponse<AdminCommentItem>> {
    tracing::debug!(
        module = "comment",
        event = "list_my_comments",
        user_id = %auth.id,
        page = query.page.unwrap_or(1),
        page_size = query.page_size.unwrap_or(20),
        "listing user comments"
    );
    let pagination = PaginationQuery {
        page: query.page,
        page_size: query.page_size,
    };
    let (page, page_size, offset) = pagination.normalized(20, 100);
    let items = repository::list_by_user(&state.pool, &auth.id, page_size, offset).await?;
    let total = repository::count_by_user(&state.pool, &auth.id).await?;
    Ok(PaginatedResponse::new(items, page, page_size, total))
}

pub async fn delete_own_comment(
    state: Arc<AppState>,
    auth: &AuthUser,
    id: &str,
) -> AppResult<serde_json::Value> {
    let deleted = repository::soft_delete_owned(&state.pool, id, &auth.id).await?;
    if !deleted {
        tracing::warn!(
            module = "comment",
            event = "delete_own_not_found",
            comment_id = %id,
            user_id = %auth.id,
            "own comment delete rejected"
        );
        return Err(AppError::NotFound);
    }
    tracing::info!(
        module = "comment",
        event = "delete_own_success",
        comment_id = %id,
        user_id = %auth.id,
        "own comment deleted"
    );
    Ok(serde_json::json!({ "deleted": true }))
}

pub async fn list_admin_comments(
    state: Arc<AppState>,
    query: CommentQuery,
) -> AppResult<PaginatedResponse<AdminCommentItem>> {
    let pagination = PaginationQuery {
        page: query.page,
        page_size: query.page_size,
    };
    let (page, page_size, offset) = pagination.normalized(20, 100);
    let items =
        repository::list_admin(&state.pool, query.status.as_deref(), page_size, offset).await?;
    let total = repository::count_admin(&state.pool, query.status.as_deref()).await?;
    Ok(PaginatedResponse::new(items, page, page_size, total))
}

pub async fn approve_comment(state: Arc<AppState>, id: &str) -> AppResult<serde_json::Value> {
    // 先查数据再更新，以便 WS 事件包含完整信息
    let comment = repository::find_by_id_for_event(&state.pool, id)
        .await?
        .ok_or(AppError::NotFound)?;

    repository::update_status(&state.pool, id, "approved").await?;

    // WS：前台文章页实时显示审核通过的评论
    let event = ServerEvent::CommentApproved {
        id: comment.id,
        post_id: comment.post_id,
        author_name: comment.author_name,
        content: comment.content,
        created_at: comment.created_at,
    };
    let _ = state.event_tx.send(event);

    Ok(serde_json::json!({ "approved": true }))
}

pub async fn reject_comment(state: Arc<AppState>, id: &str) -> AppResult<serde_json::Value> {
    // reject 暂无专门 WS 事件（仅管理后台可见，管理员主动刷新即可）
    repository::update_status(&state.pool, id, "rejected").await?;
    Ok(serde_json::json!({ "rejected": true }))
}

pub async fn delete_comment(state: Arc<AppState>, id: &str) -> AppResult<serde_json::Value> {
    repository::soft_delete_admin(&state.pool, id).await?;

    // WS：通知管理后台评论已删除
    let event = ServerEvent::CommentDeleted { id: id.to_string() };
    let _ = state.event_tx.send(event);

    Ok(serde_json::json!({ "deleted": true }))
}

pub async fn restore_comment(state: Arc<AppState>, id: &str) -> AppResult<serde_json::Value> {
    let restored = repository::restore_deleted_admin(&state.pool, id).await?;
    if !restored {
        return Err(AppError::NotFound);
    }
    Ok(serde_json::json!({ "restored": true }))
}

pub async fn purge_comment(state: Arc<AppState>, id: &str) -> AppResult<serde_json::Value> {
    let purged = repository::purge_admin(&state.pool, id).await?;
    if !purged {
        return Err(AppError::NotFound);
    }
    Ok(serde_json::json!({ "purged": true }))
}
