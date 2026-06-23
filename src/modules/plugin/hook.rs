use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::shared::error::AppResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookType {
    Filter,
    Action,
}

pub struct Hook {
    pub name: String,
    pub priority: i32,
    pub plugin_name: String,
    pub hook_type: HookType,
    pub handler: Arc<dyn HookHandler>,
}

#[async_trait]
pub trait HookHandler: Send + Sync {
    async fn run(&self, ctx: &mut HookContext) -> AppResult<()>;
}

impl Hook {
    pub fn new_filter(
        name: &str,
        priority: i32,
        plugin_name: &str,
        handler: Arc<dyn HookHandler>,
    ) -> Self {
        Self {
            name: name.to_string(),
            priority,
            plugin_name: plugin_name.to_string(),
            hook_type: HookType::Filter,
            handler,
        }
    }

    pub fn new_action(
        name: &str,
        priority: i32,
        plugin_name: &str,
        handler: Arc<dyn HookHandler>,
    ) -> Self {
        Self {
            name: name.to_string(),
            priority,
            plugin_name: plugin_name.to_string(),
            hook_type: HookType::Action,
            handler,
        }
    }
}

impl Clone for Hook {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            priority: self.priority,
            plugin_name: self.plugin_name.clone(),
            hook_type: self.hook_type,
            handler: self.handler.clone(),
        }
    }
}

pub struct HookContext {
    pub hook_name: String,
    pub data: HookData,
}

impl Clone for HookContext {
    fn clone(&self) -> Self {
        Self {
            hook_name: self.hook_name.clone(),
            data: self.data.clone(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum HookData {
    PostBeforeSave(PostBeforeSaveData),
    PostAfterSave(PostAfterSaveData),
    PostAfterPublish(PostAfterPublishData),
    PostBeforeRender(PostBeforeRenderData),
    CommentBeforeCreate(CommentBeforeCreateData),
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PostBeforeSaveData {
    pub title: String,
    pub content_html: String,
    pub excerpt: Option<String>,
    pub slug: String,
    pub tags: Vec<String>,
    pub category_id: Option<String>,
    pub content_type: String,
    pub request_ip: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PostAfterSaveData {
    pub post_id: String,
    pub title: String,
    pub slug: String,
    pub is_new: bool,
    pub status: String,
    pub old_status: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PostAfterPublishData {
    pub post_id: String,
    pub title: String,
    pub slug: String,
    pub old_status: String,
    pub new_status: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PostBeforeRenderData {
    pub post_id: String,
    pub title: String,
    pub slug: String,
    pub content_html: String,
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CommentBeforeCreateData {
    pub content: String,
    pub author_name: String,
    pub author_email: Option<String>,
    pub post_id: String,
    pub post_title: String,
    pub request_ip: Option<String>,
}
