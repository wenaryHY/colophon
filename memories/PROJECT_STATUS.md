# InkForge 项目现状（唯一真相来源）

**最后更新**: 2026-05-29  
**版本**: v0.3.6-dev  
**状态**: 开发中（架构演进 Phase 1/2/3 已完成，Tauri 壳已可编译）

---

## 一句话结论

InkForge 是一个 **Web 优先单体 CMS**。技术债已全部收口，核心测试网络已建立，当前重心在打造纯净的桌面端 In-Process 运行壳，以铺垫未来的生态扩展。

---

## 当前技术基线

| 层 | 技术 | 状态 |
|---|---|---|
| 后端 | Rust + Axum 0.7 + sqlx 0.7 + SQLite | 已落地 |
| 前端 | React 19 + TypeScript + Vite 8 | 已落地 |
| 编辑器 | Tiptap + CodeMirror 6 | 已落地 |
| 模板渲染 | MiniJinja | 已落地 |
| 模板缓存 | TemplateContextCache（30s TTL + RwLock 双重检查） | 已落地 |
| 搜索 | SQLite FTS5 | 已落地 |
| 桌面壳 | Tauri 2 (In-Process 单进程模式) | 核心重构完成，进入打包验证 |
| 测试基建 | Playwright (前端E2E) + 原生 Cargo test (后端集成) | 已落地并覆盖核心链路 |

---

## 已验证落地的能力

### 内容与后台
- ✅ 认证与授权（JWT + Argon2）
- ✅ `post | page` 双内容类型
- ✅ `page_render_mode`（`editor | custom_html`）
- ✅ Tiptap 编辑器 + CodeMirror 源码模式
- ✅ `/api/v1/...` 正式接口 + legacy `/api/...` 兼容层
- ✅ React 管理后台与中英文切换

### 安装向导
- ✅ `setup` 模块（handler / service / repository）
- ✅ 四阶段状态模型：`not_started / admin_created / configured / completed`
- ✅ 空库时 `/` 与 `/admin` 自动引导到 `/setup`
- ✅ Web 首装流程：站点信息、管理员初始化、后台入口配置
- ✅ 安装完成后的运行态缓存刷新与旧库状态回填

### 主题与前台
- ✅ 主题扫描、详情、激活、配置保存
- ✅ 主题 ZIP 上传与路径校验
- ✅ MiniJinja 前台主题渲染
- ✅ Sitemap、Robots、Meta / OpenGraph / JSON-LD 接入
- ✅ 默认主题前台评论实时更新链路

### 媒体、备份、运维
- ✅ 媒体上传、分类、重命名、删除
- ✅ 本地备份、恢复、调度、下载、删除
- ✅ 统一回收站与过期清理调度
- ✅ WebSocket 管理员 / 公开事件通道
- ✅ `/api/v1/health` 与 `/api/v1/version`

---

## 已完成的安全基线

- ✅ 生产环境默认 JWT secret 可阻断
- ✅ 管理端已切换到 HttpOnly Cookie Session
- ✅ 登录链路已有内存级限流器
- ✅ 通用安全响应头中间件已接入
- ✅ 主题 ZIP 解压已做 `enclosed_name()` 路径保护
- ✅ Markdown 渲染输出已接入服务端 sanitize
- ✅ 公开注册已复用 setup 完成态守卫
- ✅ 后台内部导航已收敛到 `/admin/*`
- ✅ 公开搜索已修复 `tag_id` 过滤与 `pagination.total`
- ✅ 全局 request_id 中间件已接入

---

## 当前技术债清单

### 高优先级（应立即处理）

| 问题 | 现状 | 建议 |
|------|------|------|
| ~~数据库迁移入口残留~~ | ~~`src/db.rs` 和 `src/infra/db/pool.rs` 仍保留旧迁移入口~~ | ✅ 已删除 |
| ~~编译优化缺失~~ | ~~`Cargo.toml` 缺少 `lto`、`opt-level` 配置~~ | ✅ 已添加 |
| ~~legacy `/api` 兼容层~~ | ~~默认主题已迁移到 `/api/v1`，但 legacy 层尚未摘除~~ | ✅ 已摘除 |

### 中优先级（验收闭环）

| 问题 | 现状 | 建议 |
|------|------|------|
| ~~Tauri 打包未启用~~ | ~~`tauri.conf.json` 中 `bundle.active: false`~~ | ✅ 已启用并配置图标 |
| ~~CI 缺位~~ | ~~`.github/workflows/` 为空~~ | ✅ 已建立最小 CI |
| ~~测试覆盖率低~~ | ~~后端仅有基础测试，前端无测试~~ | ✅ 已补齐 Playwright E2E 与后端集成测试 |

### 低优先级（未来演进）

| 问题 | 现状 | 建议 |
|------|------|------|
| JWT vs PASETO | 当前使用 JWT | 可考虑迁移到 PASETO（更安全） |
| SQLite 无加密 | 当前使用普通 SQLite | 可考虑 SQLCipher（如有加密需求） |
| ~~Argon2 同步阻塞~~ | ~~当前哈希计算是同步调用~~ | ✅ 已使用 `spawn_blocking` 异步化 |

---

## 架构审计结论（2026-04-10）

### 总体评级
- **架构方向**：良好
- **实现完整度**：中高
- **边界一致性**：中
- **验收成熟度**：中低

### 已验证的架构优点
1. **后端分层方向成立** - `handler -> service -> repository` 分层清晰
2. **setup 已成为唯一安装逻辑来源** - 状态机、初始化、缓存回填完整
3. **安全基线已有真实落地** - 非空壳，已进入主链路
4. **主题与前台渲染边界清晰** - MiniJinja SSR 边界稳定
5. **备份与运维能力已进入主链路** - 快照、打包、调度完整
6. **可观测性已有起点** - tracing、health、version、request_id

### 主要风险项
1. **P0/P1 - setup/register 边界** - 已修复 ✅
2. **P1 - 管理后台一致性** - 已修复 ✅
3. **P1 - 搜索契约失真** - 已修复 ✅
4. **P1 - Tauri 字段契约错位** - 已修复 ✅
5. **P2 - legacy `/api` 兼容层** - 已摘除 ✅
6. **P2 - 自动化测试与 CI 缺位** - 已建立 ✅

---

## 近期优先级

1. ~~**删除旧迁移入口残留**（`src/db.rs`、`src/infra/db/pool.rs`）~~ ✅ 已完成
2. ~~**添加编译优化配置**~~ ✅ 已完成
3. ~~**建立最小 CI**~~ ✅ 已完成
4. ~~**摘除 legacy `/api` 兼容层**~~ ✅ 已完成
5. ~~**启用 Tauri 打包配置**~~ ✅ 已完成
6. ~~**补充主链路回归测试**~~ ✅ 已完成
7. ~~**Tauri 启动模型改造 (In-Process)**~~ ✅ 已完成
8. **在桌面端验收稳定前，不继续前推插件系统**
9. ~~**消除 block_in_place 死锁风险 + 模板预提取底座**~~ ✅ 已完成（架构演进 Phase 1）
10. ~~**模板引擎 trait 解耦 + TTL 缓存**~~ ✅ 已完成（架构演进 Phase 2）
11. **Tauri 壳：已可编译，待修复 .ico 打包后即可全平台分发**
12. ~~**插件系统（Phase 3）: Plugin trait + PluginManager + HelloWorld demo**~~ ✅ 已完成（2026-05-29）

---

## 仓库事实

- 默认后端端口：`2000`
- 默认 Vite 开发端口：`5173`
- 迁移文件：`001` 到 `013`
- 真实迁移入口：`src/main.rs` 中的 `sqlx::migrate!("./migrations")`
- `src-tauri/` 属于实验性骨架，非稳定交付物

---

## 文档导航

- 想知道**为什么这样设计**，看 `architecture-decisions.md`
- 想知道**接下来怎么推进**，看 `executable-roadmap.md`
- 想知道**运行规则与协作规范**，看 `RUNTIME_RULES.md`