use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use super::context::TemplateContext;
use super::provider::TemplateDataProvider;
use crate::shared::error::AppResult;

const DEFAULT_CACHE_TTL_SECS: u64 = 30;

struct CacheEntry {
    context: TemplateContext,
    created_at: Instant,
}

pub struct TemplateContextCache {
    entry: Arc<RwLock<Option<CacheEntry>>>,
    ttl: Duration,
}

impl TemplateContextCache {
    pub fn new(ttl_secs: u64) -> Self {
        Self {
            entry: Arc::new(RwLock::new(None)),
            ttl: Duration::from_secs(ttl_secs),
        }
    }

    pub fn with_default_ttl() -> Self {
        Self::new(DEFAULT_CACHE_TTL_SECS)
    }

    pub async fn get_or_load(
        &self,
        provider: &dyn TemplateDataProvider,
    ) -> AppResult<TemplateContext> {
        {
            let read_guard = self.entry.read().await;
            if let Some(entry) = read_guard.as_ref() {
                if entry.created_at.elapsed() < self.ttl {
                    return Ok(entry.context.clone());
                }
            }
        }

        let mut write_guard = self.entry.write().await;
        if let Some(entry) = write_guard.as_ref() {
            if entry.created_at.elapsed() < self.ttl {
                return Ok(entry.context.clone());
            }
        }

        let context = TemplateContext::from_provider(provider).await?;
        *write_guard = Some(CacheEntry {
            context: context.clone(),
            created_at: Instant::now(),
        });
        Ok(context)
    }

    pub async fn invalidate(&self) {
        let mut write_guard = self.entry.write().await;
        *write_guard = None;
    }
}
