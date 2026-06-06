# InkForge

[![English](https://img.shields.io/badge/lang-English-blue)](README.md)
[![中文](https://img.shields.io/badge/lang-中文-ff6b35)](README.zh-CN.md)

> 一个文件就能跑的 CMS。
> 不需要 Node.js。不需要运行时。不需要 Docker。
> `scp` 到服务器上就完事了。

## 快速开始

```bash
# 前置条件：Rust 1.75+、Node.js 22+、SQLite 3
git clone https://github.com/wenaryHY/inkforge.git
cd inkforge
cd src/admin/ui && npm ci && cd -
cargo build --release -p inkforge
cargo run --release
# → http://localhost:2000/admin — 创建你的管理员账号
```

> 📖 **完整文档**：[docs/quickstart.md](docs/quickstart.md) — 15 分钟上手指南

首次启动时，InkForge 会在浏览器中打开安装向导。选择一个管理员用户名和密码，挑一款主题，60 秒内即可开始写作。前端资源已预构建并嵌入单一二进制文件中——不需要反向代理，不需要独立的 Node 进程。

## 为什么选择 InkForge？

InkForge 是一款博客平台，基于一个信念构建：你的内容技术栈不应该需要一支 DevOps 团队。大多数 CMS 平台运行在 Node.js 或 PHP 上，在运行时拉入几十个依赖，空闲时占用 150–300 MB 内存。InkForge 编译为单个静态二进制文件，在单一进程中同时服务你的博客、管理后台和 API，内存占用不到 20 MB。

性能不是事后的补丁——它是地基。从 TLS 终止到 SQLite 查询，整个请求路径都运行在 Rust 异步运行时内，零 GC 停顿。这意味着在入门级 VPS 硬件上，即使使用默认的 SQLite WAL 模式配置，也能达到 p95 低于 10ms 的响应时间。不需要 Redis，不需要 opcache，不需要调优。

插件系统在设计层面就是编译期安全的。插件是实现某个 trait 的 Rust crate——编译器会在你的站点启动之前验证类型安全和 API 契约。要禁用某个插件，在管理后台翻转一个布尔值，该插件便从请求路径中消失。没有运行时动态分发开销，没有 `eval`，没有猴子补丁。

## 功能特性

- **文章和页面双内容类型** — 双类型体系；页面可同时承载 Markdown 正文和自定义 HTML
- **双模式编辑器** — Tiptap 所见即所得 + CodeMirror 源码模式，一键切换
- **Web 安装向导** — 首次运行安装流程，含状态回填和管理路径配置
- **层级分类和标签** — 嵌套分类树，支持多标签关联
- **带审核的评论系统** — 审核队列，WebSocket 实时推送
- **媒体库** — 本地存储，按分类组织，支持图片和音频
- **统一认证** — Argon2 密码哈希，JWT + Session Cookie + API Key，7 天持久登录
- **主题引擎** — MiniJinja 模板，支持可视化配置面板和 ZIP 上传
- **实时预览** — FAB 浮动按钮触发，支持 inline / modal / 新标签页三种预览模式；可切换主题预览
- **全文搜索** — SQLite FTS5 增量索引
- **统一回收站** — 文章、分类、标签、评论共用回收站，定时清理
- **SEO 工具集** — 自动生成 sitemap、robots.txt、OpenGraph 和 JSON-LD 元数据
- **Webhook 回调** — 文章发布和更新事件 HTTP 通知，可按 URL 分别配置
- **备份与恢复** — 本地备份，一键还原，cron 定时快照
- **API 版本化** — `/api/v1/` 稳定路由，旧路由兼容过渡
- **响应式管理后台** — 三断点侧边栏，移动端卡片化表格布局，可折叠编辑面板
- **i18n** — 管理后台支持多语言
- **插件系统** — 基于 Rust trait：自定义 API 路由、模板函数、前端资源、生命周期钩子、设置面板、UI 插槽
- **单二进制部署** — WSL 交叉编译流水线，产出一个含二进制和资源的 tar 包
- **数据库抽象** — `SqlitePool` 封装于 `Executor` trait 之后，支持可测试性和后端可移植性

## 性能

| | InkForge | Ghost | WordPress |
|---|---|---|---|
| **语言** | Rust | Node.js | PHP |
| **响应时间 (p95)** | <10ms | ~50ms | ~200ms |
| **空闲内存** | ~20MB | ~150MB | ~256MB |
| **月均 VPS 费用** | $6 | $15 | $20 |

测试环境：$6/月 VPS（1 vCPU，1 GB RAM），默认主题，100 篇缓存文章。响应时间为服务端 p95；端到端延迟取决于 CDN 和网络。InkForge 可在最小的 DigitalOcean droplet 上舒适运行；Ghost 和 WordPress 通常需要更高一档配置才能达到相当的可靠性。

## 架构

- **后端：** Rust + Axum 0.8 + SQLite（WAL 模式，通过 `sqlx`）
- **前端：** React 19 + TypeScript + Vite 8，构建时嵌入
- **认证：** JWT + 刷新令牌 + Argon2 哈希 + API Key（用于 headless CMS 访问）
- **插件：** 编译期注册（通过 `build.rs` 自动发现）+ 运行时启用/禁用开关
- **Webhook：** 文章生命周期事件触发的 HTTP POST 回调，含重试和超时机制
- **主题：** MiniJinja 模板引擎；主题为含 `theme.toml` 清单的 ZIP 归档
- **搜索：** SQLite FTS5 虚拟表，内容变更时增量重建
- **桌面壳：** Tauri 2 进程内模式，与 Web 服务共享同一 `lib.rs` 入口

## 插件示例

一个在文章发布时记录日志的最小插件。在 `plugins/hello-world/` 目录下创建两个文件：

**`plugin.toml`**

```toml
[plugin]
id = "hello-world"
title = "Hello World"
version = "0.1.0"
description = "Logs a message when a post is published"
author = "You"

[engine]
inkforge = ">=1.0.0"

[hooks]
template = false
routes = false
assets = []
```

**`src/lib.rs`**

```rust
use async_trait::async_trait;
use std::sync::Arc;

use crate::modules::plugin::hook::{Hook, HookContext, HookData, HookHandler};
use crate::modules::plugin::Plugin;
use crate::shared::error::AppResult;

pub struct HelloPlugin;

impl HelloPlugin {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl Plugin for HelloPlugin {
    fn name(&self) -> &str { "hello-world" }
    fn version(&self) -> &str { "0.1.0" }

    fn hooks(&self) -> Vec<Hook> {
        struct PublishLogger;

        #[async_trait]
        impl HookHandler for PublishLogger {
            async fn run(&self, ctx: &mut HookContext) -> AppResult<()> {
                if let HookData::PostAfterPublish(ref data) = ctx.data {
                    tracing::info!(
                        "Post published: {} (slug: {})",
                        data.title,
                        data.slug,
                    );
                }
                Ok(())
            }
        }

        vec![Hook::new_action(
            "post.after_publish",
            10,
            self.name(),
            Arc::new(PublishLogger),
        )]
    }
}
```

重新构建项目——`build.rs` 会自动发现插件目录并将其链接到二进制中。可在管理后台运行时启用或禁用它。

## 部署

### 一键命令（Linux VPS，通过 WSL）

```bash
bash deploy-fast.sh
```

在 WSL 内本地构建前端和 Rust 二进制，通过 `scp` 将两者上传到服务器，备份数据库，替换二进制，并重启 systemd 服务。脚本退出前会执行健康检查以确认部署成功。服务端前置准备（用户、数据目录、systemd unit）请参见 `docs/DEPLOY.md`。

### Docker

```bash
docker build -t inkforge .
docker run -d \
  -p 3000:3000 \
  -e INKFORGE__AUTH__SECRET="$(openssl rand -hex 32)" \
  -v inkforge-uploads:/app/uploads \
  -v inkforge-backups:/app/backups \
  -v inkforge-data:/app/data \
  inkforge
```

镜像内置 Litestream，用于将 SQLite 持续复制到 S3 兼容存储。在 `config/litestream.yml` 中配置复制策略。

### 二进制

从 [Releases](https://github.com/wenaryHY/inkforge/releases) 页面下载预构建二进制，或从源码构建：

```bash
cd src/admin/ui && npm ci && npm run build && cd -
cargo build --release -p inkforge
```

将 `target/release/inkforge`、`config/` 目录、`migrations/` 和 `themes/` 复制到你的服务器。直接运行二进制——除 `libsqlite3` 外无运行时依赖。

## 安全

- **暴力破解防护：** 通过 `governor` 实现登录速率限制，可配置突发和每秒配额
- **密码存储：** Argon2id 哈希，随机每密码盐值
- **会话管理：** HTTP-only 安全 Cookie，7 天过期，服务端可撤销
- **API Key：** 用于 headless CMS 访问的限定范围密钥，可从管理后台吊销
- **垃圾评论防护：** 内置蜜罐字段，可选 Cloudflare Turnstile 集成
- **内容净化：** 用户提交的 HTML 在渲染前通过 `ammonia` 清洗
- **依赖审计：** 每次 `cargo audit` 对完整依赖树运行（最新结果见下方安全审计章节）

## 对比

InkForge 适合个人博客、开发者作品集、文档站点，以及速度与低运营成本比第三方集成生态更重要的中小型出版物。

Ghost 提供更成熟的管理体验、内置会员和 newsletter 系统，以及更大的主题市场。如果你今天就需要订阅计费或多作者新闻编辑室工作流，Ghost 是更安全的选择。不过，Ghost 运行在 Node.js 上，空闲内存约为 InkForge 的 7–8 倍。

WordPress 拥有所有 CMS 中规模最大的插件生态，高出数量级。如果你的站点依赖某个特定的 WooCommerce 扩展、页面构建器或深层 SEO 插件链，WordPress 是务实的选择。代价是运行时开销和攻击面——WordPress 站点需要定期打补丁、PHP opcache 层，以及通常需要单独的反向代理缓存才能达到 InkForge 开箱即得的响应时间。

InkForge 的插件系统是 Rust 原生的：插件被编译、静态链接，并在部署前由类型系统验证。这与 PHP 或 JavaScript 的插件模型有本质区别——默认更安全，但插件编写的门槛更高。

## 许可证

**AGPLv3**（从 v1.0.0 起）。见 [LICENSE](LICENSE)。

你可以根据 AGPLv3 条款免费自托管 InkForge。如果你希望将 InkForge 作为商业 SaaS 提供而不公开你的修改，请联系作者讨论替代许可。

## 路线图

### 当前（2026 年 Q2）

- [x] 文章生命周期操作追踪
- [x] Webhook 可靠性改进（重试逻辑）
- [ ] 移动端编辑器 UX 打磨
- [ ] 英文文档站点

### 下一步（2026 年 Q3）

- [ ] 多语言内容支持（按文章设置 locale）
- [ ] 主题市场（一键安装）
- [ ] 托管服务早期体验

### 远期

- [ ] 通过 `custom_fields` JSON 列实现自定义内容类型
- [ ] GraphQL API 与 REST 并存
- [ ] 图片懒加载（blur-up 占位图）

## 安全审计

`cargo audit` 扫描 760 个 crate（2026-06-02）。**服务器二进制中无关键漏洞。**

| 漏洞 | 严重度 | 路径 | 状态 |
|---|---|---|---|
| `sqlx` 二进制协议溢出 | 中 | sqlx-mysql / sqlx-postgres（SQLite 不受影响） | 无害 |
| `rsa` 时序侧信道 | 中 | rsa → sqlx-mysql（仅 MySQL） | 无害 |
| `rustls-webpki` CRL panic | 高 | aws-sdk-s3（当前未启用） | 无害 |
| `rustls-webpki` 通配符证书 | 高 | aws-sdk-s3（当前未启用） | 无害 |
| `glib` 不安全迭代器 | 未定义 | tauri → webkit2gtk（仅桌面端） | 无害 |
| `lru` 悬空指针 | 未定义 | aws-sdk-s3（当前未启用） | 无害 |
| `rand` 自定义 logger | 未定义 | tauri-utils（仅桌面端） | 无害 |
| 12 个未维护警告 | — | gtk-rs crate（仅桌面端） | 无害 |

**启用以下功能前，请先升级对应依赖：**

- **PostgreSQL / MySQL 后端** → 升级 `sqlx` 到 ≥0.8.1
- **S3 对象存储** → 升级 `rustls-webpki` 到 ≥0.103.13
- **Tauri 桌面壳** → 升级整个 Tauri 工具链

## 参与贡献

欢迎提交 Pull Request。请尽量使用英文撰写 commit message 和文档。完整指南见 [CONTRIBUTING.md](CONTRIBUTING.md)。
