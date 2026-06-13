#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use std::sync::Arc;
    use tokio::time::{sleep, Duration};

    use crate::modules::plugin::hook::{
        Hook, HookContext, HookData, HookHandler, PostAfterPublishData,
    };
    use crate::modules::plugin::hook_registry::HookRegistry;
    use crate::shared::error::AppResult;

    struct CountingHook {
        counter: Arc<AtomicU32>,
    }

    #[async_trait]
    impl HookHandler for CountingHook {
        async fn run(&self, _ctx: &mut HookContext) -> AppResult<()> {
            self.counter.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    struct SlowHook {
        ms: u64,
    }

    #[async_trait]
    impl HookHandler for SlowHook {
        async fn run(&self, _ctx: &mut HookContext) -> AppResult<()> {
            sleep(Duration::from_millis(self.ms)).await;
            Ok(())
        }
    }

    struct FailingHook;

    #[async_trait]
    impl HookHandler for FailingHook {
        async fn run(&self, _ctx: &mut HookContext) -> AppResult<()> {
            Err(anyhow::anyhow!("simulated failure").into())
        }
    }

    fn make_ctx() -> HookContext {
        HookContext {
            hook_name: "test.hook".into(),
            data: HookData::PostAfterPublish(PostAfterPublishData {
                post_id: "test-id".into(),
                title: "Test".into(),
                slug: "test".into(),
                old_status: "draft".into(),
                new_status: "published".into(),
            }),
        }
    }

    #[tokio::test]
    async fn register_and_dispatch_filter() {
        let registry = HookRegistry::new();
        let counter = Arc::new(AtomicU32::new(0));
        let hook = Hook::new_filter(
            "test.hook",
            10,
            "test-plugin",
            Arc::new(CountingHook {
                counter: counter.clone(),
            }),
        );
        registry.register("test-plugin", vec![hook]).await;

        let mut ctx = make_ctx();
        registry
            .dispatch_filter("test.hook", &mut ctx)
            .await
            .unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn multiple_hooks_executed_in_priority_order() {
        let registry = HookRegistry::new();
        let order = Arc::new(tokio::sync::Mutex::new(Vec::new()));

        struct PriorityHook {
            order: Arc<tokio::sync::Mutex<Vec<i32>>>,
            id: i32,
        }

        #[async_trait]
        impl HookHandler for PriorityHook {
            async fn run(&self, _ctx: &mut HookContext) -> AppResult<()> {
                self.order.lock().await.push(self.id);
                Ok(())
            }
        }

        let hook_a = Hook::new_filter(
            "test.hook",
            20,
            "p-a",
            Arc::new(PriorityHook {
                order: order.clone(),
                id: 2,
            }),
        );
        let hook_b = Hook::new_filter(
            "test.hook",
            5,
            "p-b",
            Arc::new(PriorityHook {
                order: order.clone(),
                id: 1,
            }),
        );
        registry.register("a", vec![hook_a]).await;
        registry.register("b", vec![hook_b]).await;

        let mut ctx = make_ctx();
        registry
            .dispatch_filter("test.hook", &mut ctx)
            .await
            .unwrap();
        let executed = order.lock().await.clone();
        assert_eq!(executed, vec![1, 2], "lower priority should execute first");
    }

    #[tokio::test]
    async fn filter_failure_stops_pipeline() {
        let registry = HookRegistry::new();
        let counter = Arc::new(AtomicU32::new(0));
        let fail = Hook::new_filter("test.hook", 5, "fail", Arc::new(FailingHook));
        let after = Hook::new_filter(
            "test.hook",
            10,
            "after",
            Arc::new(CountingHook {
                counter: counter.clone(),
            }),
        );
        registry.register("test", vec![fail, after]).await;

        let mut ctx = make_ctx();
        let result = registry.dispatch_filter("test.hook", &mut ctx).await;
        assert!(result.is_err());
        assert_eq!(
            counter.load(Ordering::SeqCst),
            0,
            "hook after failure should not execute"
        );
    }

    #[tokio::test]
    async fn dispatch_filter_best_effort_skips_failures() {
        let registry = HookRegistry::new();
        let counter = Arc::new(AtomicU32::new(0));
        let fail = Hook::new_filter("test.hook", 5, "fail", Arc::new(FailingHook));
        let ok = Hook::new_filter(
            "test.hook",
            10,
            "ok",
            Arc::new(CountingHook {
                counter: counter.clone(),
            }),
        );
        registry.register("test", vec![fail, ok]).await;

        let mut ctx = make_ctx();
        registry
            .dispatch_filter_best_effort("test.hook", &mut ctx)
            .await;
        assert_eq!(
            counter.load(Ordering::SeqCst),
            1,
            "ok hook should still execute after failure in best_effort mode"
        );
    }

    #[tokio::test]
    async fn dispatch_action_is_fire_and_forget() {
        let registry = HookRegistry::new();
        let flag = Arc::new(AtomicBool::new(false));

        struct FlagHook {
            flag: Arc<AtomicBool>,
            ms: u64,
        }

        #[async_trait]
        impl HookHandler for FlagHook {
            async fn run(&self, _ctx: &mut HookContext) -> AppResult<()> {
                sleep(Duration::from_millis(self.ms)).await;
                self.flag.store(true, Ordering::SeqCst);
                Ok(())
            }
        }

        let hook = Hook::new_action(
            "test.hook",
            10,
            "test",
            Arc::new(FlagHook {
                flag: flag.clone(),
                ms: 100,
            }),
        );
        registry.register("test", vec![hook]).await;

        let ctx = make_ctx();
        registry.dispatch_action("test.hook", ctx).await;
        assert!(
            !flag.load(Ordering::SeqCst),
            "action should not block caller"
        );
        sleep(Duration::from_millis(200)).await;
        assert!(flag.load(Ordering::SeqCst), "action should complete async");
    }

    #[tokio::test]
    async fn action_timeout_does_not_crash() {
        let registry = HookRegistry::new();
        let hook = Hook::new_action("test.hook", 10, "slow", Arc::new(SlowHook { ms: 7000 }));
        registry.register("test", vec![hook]).await;

        let ctx = make_ctx();
        registry.dispatch_action("test.hook", ctx).await;
        sleep(Duration::from_millis(200)).await;
        // no crash = pass
    }

    #[tokio::test]
    async fn unregister_all_removes_hooks() {
        let registry = HookRegistry::new();
        let counter = Arc::new(AtomicU32::new(0));
        let hook = Hook::new_filter(
            "test.hook",
            10,
            "test",
            Arc::new(CountingHook {
                counter: counter.clone(),
            }),
        );
        registry.register("test", vec![hook]).await;
        registry.unregister_all("test").await;

        let mut ctx = make_ctx();
        registry
            .dispatch_filter("test.hook", &mut ctx)
            .await
            .unwrap();
        assert_eq!(
            counter.load(Ordering::SeqCst),
            0,
            "unregistered hook should not fire"
        );
    }

    #[tokio::test]
    async fn dispatch_with_no_registered_hooks_does_not_error() {
        let registry = HookRegistry::new();
        let mut ctx = make_ctx();
        let result = registry.dispatch_filter("nonexistent.hook", &mut ctx).await;
        assert!(
            result.is_ok(),
            "dispatching without registered hooks should be safe"
        );
    }
}
