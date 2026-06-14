# Colophon

[![English](https://img.shields.io/badge/lang-English-blue)](README.md)
[![中文](https://img.shields.io/badge/lang-中文-ff6b35)](README.zh-CN.md)

为 $6 VPS 打造的 CMS。单二进制文件，单文件备份，空闲内存 <20 MB。不需要 Node 运行时，不需要反向代理，不需要 Docker。

## 快速开始

**一行安装（Linux VPS）：**

```bash
curl -fsSL https://raw.githubusercontent.com/wenaryHY/colophon/master/scripts/install.sh | bash
```

打开 `http://YOUR_IP:2000/admin` —— 首次启动时运行安装向导。支持 Debian/Ubuntu（apt）和 Fedora/CentOS（dnf/yum），x86_64 和 aarch64 架构。

**从源码构建（3 条命令）：**

```bash
git clone https://github.com/wenaryHY/colophon.git && cd colophon
cd src/admin/ui && npm ci && cd - && cargo build --release -p colophon
cargo run --release
```

需要 Rust 1.75+、Node.js 22+、SQLite 3。管理后台完全内嵌在二进制文件中 —— 不需要单独的前端服务器。

**Docker：**

```bash
docker build -t colophon .
docker run -d -p 3000:3000 \
  -e COLOPHON__AUTH__SECRET="$(openssl rand -hex 32)" \
  -v colophon-data:/app/data \
  colophon
```

Docker 镜像内置 Litestream，用于将 SQLite 持续复制到 S3 兼容存储。

## 为什么选择 Colophon

大多数 CMS 平台运行在 Node.js 或 PHP 上，拉入几十个运行时依赖，空闲时占用 150--500 MB 内存。Colophon 编译为单个静态二进制文件，在单一进程中同时服务站点、管理后台和 API，**内存占用不到 20 MB**。

从 TLS 终止到 SQLite 查询，整个请求路径运行在 Rust 异步运行时内，零 GC 停顿。这意味着在入门级 VPS 硬件上，即使使用默认的 SQLite WAL 模式，也能达到 **p95 低于 10ms 的响应时间**。不需要 Redis，不需要 opcache，不需要调优。

备份生成包含数据库和媒体文件的 ZIP 归档。（路线图：Q3 将把媒体迁移到 SQLite BLOB，实现真正的单文件备份 —— 只需 `cp colophon.db`。）

## 对比

Colophon 定位于 headless CMS 和博客平台领域。下表从对自托管部署最重要的维度，将其与最常见的替代方案进行对比。

| | Colophon | Strapi | Directus | Ghost | WordPress |
|---|---|---|---|---|---|
| **语言** | Rust | Node.js | Node.js | Node.js | PHP |
| **空闲内存** | ~20 MB | ~300 MB | ~250 MB | ~150 MB | ~256 MB |
| **响应 p95** | <10 ms | -- | -- | ~50 ms | ~200 ms |
| **二进制/依赖大小** | ~14 MB（单文件） | ~500 MB（node_modules） | ~400 MB（node_modules） | ~300 MB（node_modules） | 不适用 |
| **备份** | 一个 ZIP（DB + 媒体） | DB 导出 + uploads/ | DB 导出 + uploads/ | DB + content/ | DB 导出 + wp-content/ |
| **部署** | 复制二进制，运行 | npm install，配置，node server + DB | npm install，配置，node server + DB | Ghost CLI + Node + DB | LAMP/LEMP 技术栈 |
| **最低 VPS** | 512 MB（$4/月） | 2 GB（$18/月） | 2 GB（$18/月） | 1 GB（$6/月） | 1 GB（$6/月） |
| **数据库** | SQLite（零配置） | PostgreSQL / MySQL / SQLite | PostgreSQL / MySQL / SQLite | MySQL | MySQL |
| **插件模型** | Rust trait，静态链接 | JavaScript，运行时 | JavaScript，运行时 | JavaScript，运行时 | PHP，运行时 |
| **许可证** | AGPLv3 | MIT | BSL / MIT | MIT | GPLv2 |

Colophon 的插件系统是 Rust 原生的：插件被编译、静态链接，并在部署前由类型系统验证。这与 PHP 或 JavaScript 的插件模型有本质区别 —— 默认更安全，但插件编写的门槛更高。

## 架构

```
                  ┌──────────────────────────────────┐
                  │      Axum Router（端口 2000）      │
                  └──────────────┬───────────────────┘
         ┌───────────────────────┼──────────────────────┐
         │                       │                      │
    /api/v1/*               /admin/*               /*（公开）
         │                       │                      │
   JWT / API Key          SPA 处理器            主题渲染器
    认证层              （React，内嵌）        （MiniJinja + DB）
         │                       │                      │
   处理器 -- SQLite            --           过滤钩子（预渲染）
         │                                          │
   过滤钩子（预保存）                          渲染 HTML
         │                                          │
   DB 提交                                     响应
         │
   操作钩子（即发即弃）
         │
   Webhook / 插件 / 邮件
```

- **后端：** Rust + Axum 0.8 + SQLite WAL 模式（通过 `sqlx`）
- **前端：** React 19 + TypeScript + Vite 8，构建时编译并内嵌
- **认证：** Argon2id 密码哈希，JWT 含刷新令牌，Session Cookie，API Key
- **模板：** MiniJinja 引擎；主题为包含 `theme.toml` 清单和可视化配置面板的 ZIP 归档
- **搜索：** SQLite FTS5 虚拟表，内容变更时增量重建
- **桌面端：** Tauri 2（可选），与服务端共享同一 `lib.rs` 入口

## 功能特性

### 内容

| 功能 | 描述 |
|---|---|
| 双内容类型 | 文章和页面，各自独立的 URL 命名空间 |
| 双模式编辑器 | Tiptap 所见即所得 + CodeMirror 源码模式，一键切换 |
| 层级分类 | 嵌套分类树，支持多标签关联 |
| 全文搜索 | SQLite FTS5，内容变更时增量索引 |
| SEO 工具集 | 自动生成 sitemap、robots.txt、OpenGraph 和 JSON-LD 元数据 |
| 统一回收站 | 文章、分类、标签、评论共用回收站，定时清理 |

### 媒体

| 功能 | 描述 |
|---|---|
| 媒体库 | 本地存储，按分类组织 |
| 支持格式 | 图片（WebP、PNG、JPEG、GIF、SVG）和音频（MP3、WAV、OGG） |
| 封面图片 | 每篇文章可设封面，自动生成缩略图 |

### 发布

| 功能 | 描述 |
|---|---|
| 评论系统 | 审核队列，WebSocket 实时推送 |
| Webhook 回调 | 文章生命周期事件触发 HTTP POST，含重试和超时机制 |
| 文章生命周期追踪 | 发布、更新、回收等操作的操作历史记录 |

### 安全

| 功能 | 描述 |
|---|---|
| 密码哈希 | Argon2id，每密码随机盐值 |
| 会话管理 | HTTP-only Cookie，7 天过期，服务端可撤销 |
| 暴力破解防护 | 通过 `governor` 实现登录速率限制 |
| 内容净化 | 用户提交的 HTML 通过 `ammonia` 清洗 |
| 垃圾评论防护 | 内置蜜罐字段 + 可选 Cloudflare Turnstile |

### DevOps

| 功能 | 描述 |
|---|---|
| 单二进制部署 | 一个文件 + 一个配置目录；scp 到服务器即可 |
| 一键部署脚本 | 构建、上传、备份 DB、重启服务、健康检查 |
| Docker 支持 | 官方镜像，内置 Litestream 实现 SQLite 持续复制 |
| 备份与恢复 | 本地快照，一键还原，cron 定时调度 |
| API 版本化 | `/api/v1/` 稳定路由，旧版兼容过渡 |

### 管理后台 UX

| 功能 | 描述 |
|---|---|
| 现代技术栈 | React 19 + TypeScript + Vite 8，构建时内嵌 |
| 响应式设计 | 三断点侧边栏，移动端卡片化表格布局 |
| 实时预览 | FAB 浮动按钮触发，支持 inline / modal / 新标签页三种预览模式 |
| 主题配置 | 每款主题的可视化配置面板（颜色、布局、文本选项） |
| i18n | 管理后台支持多语言 |

## 扩展

Colophon 提供两条扩展路径，针对不同技术投入水平设计。

### Webhook（零代码）

```
 ┌──────────┐   post.after_publish    ┌──────────────┐
 │ Colophon │ ──────────────────────► │  你的服务     │
 └──────────┘   HTTP POST + JSON      └──────────────┘
                                         （重建、通知、
                                          索引、归档……）
```

在管理后台配置 Webhook URL。Colophon 在每次文章生命周期事件时发送带 JSON 载荷的 HTTP POST。内置重试逻辑、并发控制和投递日志。参见 [Webhook 指南](docs/webhook-guide.md)。

### 插件（Rust，完全控制）

```
 ┌──────────┐
 │ Colophon │
 │          │   Plugin trait
 │ ┌──────┐ │   ┌─────────────┐
 │ │ Core │◄┼───┤ Plugin A    │   api_routes()     -- 自定义 REST 端点
 │ └──────┘ │   │ Plugin B    │   extend_template() -- MiniJinja 函数
 │          │   │ Plugin C    │   frontend_assets() -- CSS/JS 注入
 │ 管理后台 │   │ ...         │   hooks()           -- 生命周期过滤/操作钩子
 └──────────┘   └─────────────┘
```

插件是从 `plugins/` 目录发现的 Rust crate。它们实现 `Plugin` trait，在构建时编译并静态链接到二进制文件中 —— 无运行时动态分发开销。每个插件可以注册：

- **API 路由** —— 位于 `/api/v1/plugins/` 下的自定义 `axum::Router` 处理器
- **模板函数** —— 可在任何 MiniJinja 主题模板中调用
- **前端资源** —— 注入管理后台的 CSS/JS
- **钩子** —— 过滤钩子（同步，可修改数据）和操作钩子（即发即弃，不能阻塞响应）
- **设置** —— 在管理后台展示的用户可配置设置

可在管理后台启用或禁用插件，无需重启。参见 [插件指南](docs/plugin-guide.md)。

## 性能

在 $6/月 VPS（1 vCPU，1 GB RAM）上，使用默认主题和约 100 篇文章测得。基准测试使用 Criterion 在 Rust 1.75+ 上运行。

### 数据库查询

| 操作 | Colophon | 备注 |
|---|---|---|
| 单行查询（按 slug） | ~30.5 us | 有索引 |
| 单行查询（按 id） | ~33 us | 有索引 |
| 列出 20 篇文章 | ~124 us | SELECT with LIMIT 20 |
| 插入文章 | ~39.6 us | INSERT with indexes |

### 对比 Strapi / Directus（单行查询）

| | Colophon | Strapi | Directus |
|---|---|---|---|
| 单行查询 | ~33 us | ~500 us | ~400 us |
| 比率 | 1x | 慢 15x | 慢 12x |

### JSON 序列化

| 数据量 | 序列化 | 反序列化 |
|---|---|---|
| 1 篇文章 | ~350 ns | ~420 ns |
| 10 篇文章 | ~3.3 us | ~4.2 us |
| 100 篇文章 | ~32.0 us | ~43.3 us |

SQLite 在 WAL 模式下配合适当索引，无需外部协调即可处理并发读写。不需要连接池，不需要缓存预热，不需要读副本。

完整基准测试方法论和脚本：[benches/BASELINE.md](benches/BASELINE.md)。

## 路线图

### 当前（2026 年 Q2）

- [x] 文章生命周期操作追踪
- [x] Webhook 可靠性改进（含重试逻辑）
- [x] `colophon export` 命令 —— 导出 JSON + 媒体，用于 Astro/Next.js 静态生成
- [ ] 移动端编辑器 UX 打磨
- [ ] 英文文档站点

### 下一步（2026 年 Q3）

- [ ] 媒体资源迁移至 SQLite BLOB —— 真正的单文件备份（`cp colophon.db`）
- [ ] 多语言内容支持（按文章设置 locale）
- [ ] 主题市场（一键安装）
- [ ] 托管服务早期体验

### 远期

- [ ] 通过 `custom_fields` JSON 列实现自定义内容类型
- [ ] GraphQL API 与 REST 并存
- [ ] 图片懒加载（blur-up 占位图）

## 静态导出（自 Q2 2026 起）

`colophon export` 命令提取所有内容和媒体，用于静态站点生成：

```bash
# 导出所有内容和媒体
colophon export --output ./static-data

# 直接在你的前端构建中使用
# Astro: import posts from '../static-data/posts.json'
# Next.js: const posts = require('./static-data/posts.json')
```

## 社区

- **Issues**: [github.com/wenaryHY/colophon/issues](https://github.com/wenaryHY/colophon/issues)
- **参与贡献**: [CONTRIBUTING.md](CONTRIBUTING.md)
- **Discussions**: [github.com/wenaryHY/colophon/discussions](https://github.com/wenaryHY/colophon/discussions)

欢迎提交 Pull Request。环境搭建说明和完整工作流见 [贡献指南](CONTRIBUTING.md)。

## 许可证

**AGPLv3**（从 v1.0.0 起）。见 [LICENSE](LICENSE)。

你可以根据 AGPLv3 条款免费自托管 Colophon。如果你希望将 Colophon 作为商业 SaaS 提供而不公开你的修改，请联系作者讨论替代许可。
