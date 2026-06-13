use std::sync::Arc;

use crate::{
    shared::error::{AppError, AppResult},
    state::AppState,
};

use super::{domain::TrashItem, repository};

/// 获取回收站设置：保留天数（默认30天）
async fn get_retention_days(state: &AppState) -> i64 {
    let value =
        crate::modules::setting::repository::get_string(&state.pool, "trash_retention_days", "30")
            .await;
    match value {
        Ok(v) => v.parse::<i64>().unwrap_or(30).clamp(1, 90),
        _ => 30,
    }
}

fn compute_expires_in_days(deleted_at: &str, retention_days: i64) -> i64 {
    let now = chrono::Utc::now().naive_utc();
    if let Ok(deleted) = chrono::NaiveDateTime::parse_from_str(deleted_at, "%Y-%m-%d %H:%M:%S") {
        let expire_date = deleted + chrono::Duration::days(retention_days);
        (expire_date - now).num_days()
    } else {
        retention_days
    }
}

fn to_trash_items(
    rows: Vec<(String, String, Option<String>, String)>,
    item_type: &str,
    retention_days: i64,
) -> Vec<TrashItem> {
    rows.into_iter()
        .map(|(id, name, subtitle, deleted_at)| {
            let expires_in_days = compute_expires_in_days(&deleted_at, retention_days);
            TrashItem {
                id,
                item_type: item_type.to_string(),
                name,
                subtitle,
                deleted_at,
                expires_in_days,
            }
        })
        .collect()
}

pub async fn list_trash(
    state: Arc<AppState>,
    type_filter: Option<&str>,
) -> AppResult<Vec<TrashItem>> {
    let retention_days = get_retention_days(&state).await;
    let mut items = Vec::new();

    let types: Vec<&str> = match type_filter {
        Some(t) => vec![t],
        None => vec![
            "post",
            "category",
            "tag",
            "media",
            "media_category",
            "comment",
        ],
    };

    for t in types {
        match t {
            "post" => {
                let rows = repository::list_trashed_posts(&state.pool).await?;
                items.extend(to_trash_items(rows, "post", retention_days));
            }
            "category" => {
                let rows = repository::list_trashed_categories(&state.pool).await?;
                items.extend(to_trash_items(rows, "category", retention_days));
            }
            "tag" => {
                let rows = repository::list_trashed_tags(&state.pool).await?;
                items.extend(to_trash_items(rows, "tag", retention_days));
            }
            "media" => {
                let rows = repository::list_trashed_media(&state.pool).await?;
                items.extend(to_trash_items(rows, "media", retention_days));
            }
            "media_category" => {
                let rows = repository::list_trashed_media_categories(&state.pool).await?;
                items.extend(to_trash_items(rows, "media_category", retention_days));
            }
            "comment" => {
                let rows = repository::list_trashed_comments(&state.pool).await?;
                items.extend(to_trash_items(rows, "comment", retention_days));
            }
            _ => {}
        }
    }

    // 按删除时间降序
    items.sort_by(|a, b| b.deleted_at.cmp(&a.deleted_at));
    Ok(items)
}

pub async fn restore_item(state: Arc<AppState>, item_type: &str, id: &str) -> AppResult<()> {
    let restored = repository::restore_item(&state.pool, item_type, id).await?;
    if !restored {
        return Err(AppError::NotFound("回收站项目未找到".to_string()));
    }
    Ok(())
}

pub async fn purge_item(state: Arc<AppState>, item_type: &str, id: &str) -> AppResult<()> {
    let purged = repository::purge_item(&state.pool, item_type, id).await?;
    if !purged {
        return Err(AppError::NotFound("回收站项目未找到".to_string()));
    }
    Ok(())
}

pub async fn purge_expired(state: Arc<AppState>) -> AppResult<i64> {
    let retention_days = get_retention_days(&state).await;
    repository::purge_expired(&state.pool, retention_days).await
}
