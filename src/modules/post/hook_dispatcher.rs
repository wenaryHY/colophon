use crate::{
    modules::plugin::hook::{
        HookContext, HookData, PostAfterPublishData, PostAfterSaveData, PostBeforeSaveData,
    },
    shared::error::AppResult,
    state::AppState,
};

use super::post_types::ContentType;

/// Hook 调度前的文章字段快照，用于 post.before_save filter 钩子。
pub struct BeforeSaveHookResult {
    pub title: String,
    pub content_html: String,
    pub excerpt: Option<String>,
    pub slug: String,
    pub tags: Vec<String>,
    pub category_id: Option<String>,
    pub content_type: ContentType,
}

/// 调度 post.before_save filter 钩子，返回可能被插件修改后的字段。
pub async fn dispatch_post_before_save(
    state: &AppState,
    title: String,
    content_html: String,
    excerpt: Option<String>,
    slug: String,
    tags: Vec<String>,
    category_id: Option<String>,
    content_type: ContentType,
) -> AppResult<BeforeSaveHookResult> {
    let hook_registry = state.plugin_manager.read().await.hook_registry().clone();
    let mut save_ctx = HookContext {
        hook_name: "post.before_save".into(),
        data: HookData::PostBeforeSave(PostBeforeSaveData {
            title: title.clone(),
            content_html: content_html.clone(),
            excerpt: excerpt.clone(),
            slug: slug.clone(),
            tags: tags.clone(),
            category_id: category_id.clone(),
            content_type: content_type.to_string(),
            request_ip: None,
            user_agent: None,
        }),
    };
    hook_registry
        .dispatch_filter("post.before_save", &mut save_ctx)
        .await?;

    if let HookData::PostBeforeSave(ref data) = save_ctx.data {
        Ok(BeforeSaveHookResult {
            title: data.title.clone(),
            content_html: data.content_html.clone(),
            excerpt: data.excerpt.clone(),
            slug: data.slug.clone(),
            tags: data.tags.clone(),
            category_id: data.category_id.clone(),
            content_type: data.content_type.parse()?,
        })
    } else {
        // dispatch_filter 不应该替换 HookData variant，但做防御性处理
        Ok(BeforeSaveHookResult {
            title,
            content_html,
            excerpt,
            slug,
            tags,
            category_id,
            content_type,
        })
    }
}

/// 调度 post.after_save action 钩子（fire-and-forget）。
pub async fn dispatch_post_after_save(
    state: &AppState,
    post_id: String,
    title: String,
    slug: String,
    is_new: bool,
    status: String,
    old_status: String,
) {
    let hook_registry = state.plugin_manager.read().await.hook_registry().clone();
    let ctx = HookContext {
        hook_name: "post.after_save".into(),
        data: HookData::PostAfterSave(PostAfterSaveData {
            post_id,
            title,
            slug,
            is_new,
            status,
            old_status: Some(old_status),
        }),
    };
    hook_registry.dispatch_action("post.after_save", ctx).await;
}

/// 调度 post.after_publish action 钩子（fire-and-forget）。
pub async fn dispatch_post_after_publish(
    state: &AppState,
    post_id: String,
    title: String,
    slug: String,
    old_status: String,
    new_status: String,
) {
    let hook_registry = state.plugin_manager.read().await.hook_registry().clone();
    let ctx = HookContext {
        hook_name: "post.after_publish".into(),
        data: HookData::PostAfterPublish(PostAfterPublishData {
            post_id,
            title,
            slug,
            old_status,
            new_status,
        }),
    };
    hook_registry
        .dispatch_action("post.after_publish", ctx)
        .await;
}
