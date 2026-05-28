# Phase 1 — Tokio 死锁修复 + 模板预提取底座

**日期**: 2026-05-28  
**状态**: ✅ 已完成  
**所属**: InkForge CMS 架构演进子计划（非官方路线图阶段，为填补当前架构审计发现的 3 大缺口而设立）

---

## 目标

消除 lock_in_place 导致的 Tokio 线程池死锁风险，打下模板引擎解耦基础。

## 变更范围

| Task | 文件 | 操作 |
|------|------|------|
| T1   | src/modules/theme/context.rs (新建) + mod.rs | +75 行 |
| T2   | src/modules/theme/engine.rs | -100/+54 行 |
| T3   | src/modules/theme/handler.rs | +12 行更新 |
| T3   | src/modules/user/theme_handler.rs | +3/-3 行更新 |
| T3   | src/modules/post/handler.rs | +2/-7 行更新 |
| T4   | src/modules/theme/handler.rs | theme_slug 校验 |

## 产出

- TemplateContext 结构体 + sync fn load() 预提取 9 个字段
- uild_template_engine 从 sync fn(state, theme_name) 改为 n(ctx, theme_dir) — 纯同步
- 6 个调用方全部适配
- 路径遍历防护（canonicalize + starts_with）
- theme_slug 合法性校验（theme.toml 存在性检查）

## 验证

cargo test -p inkforge: 31 个单元测试 + 1 个集成测试全部通过 ✅
