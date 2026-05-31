# 🖋️ InkForge（墨炉）

> 用 Rust 从零构建的现代化博客平台。Axum + SQLite + React，**Web 优先单体架构**，支持单二进制部署。

## ✨ 功能特性

- 📝 **文章与页面** — 支持 `post / page` 双内容类型，页面可同时拥有 Markdown 和自定义 HTML
- ✏️ **双模式编辑器** — Tiptap 所见即所得 + CodeMirror 源码模式，自由切换
- 🧭 **安装向导** — Web 首装流程、安装状态回填、后台入口配置
- 📁 **分类 & 标签** — 层级分类，多标签关联
- 💬 **评论系统** — 审核流程，WebSocket 实时推送
- 🖼️ **媒体管理** — 本地存储，分类整理，支持图片 / 音频
- 🔐 **用户认证** — Argon2 密码加密 + JWT / Session
- 🎨 **主题系统** — MiniJinja 模板引擎，支持可视化配置与 ZIP 上传
- 🔍 **全文搜索** — FTS5 增量索引
- 🗑️ **统一回收站** — 文章 / 分类 / 标签 / 评论统一管理，定时清理
- 📡 **SEO** — Sitemap、Robots.txt、OpenGraph / JSON-LD 元数据
- 💾 **备份与恢复** — 本地备份，一键还原，定时备份调度
- 🌐 **API 版本化** — `/api/v1/` 正式路由，旧路由兼容过渡
- 🌍 **i18n** — 管理后台多语言支持
- 🧩 **插件系统** — Rust trait 插件：路由 / 模板函数 / 前端资产 / Hooks / 配置面板 / 前端插槽

## 🛠️ 技术栈

| 模块 | 技术 |
|---|---|
| Web 框架 | Axum 0.7 |
| 运行时 | Tokio |
| 数据库 | SQLite (`sqlx` 0.7) |
| 模板引擎 | MiniJinja |
| 认证 | JWT + Argon2 + Cookie Session |
| Markdown 渲染 | `pulldown-cmark` + `ammonia` |
| 全文搜索 | SQLite FTS5 |
| 管理后台 | React 19 + TypeScript + Vite 8 |
| UI 样式 | Tailwind CSS v4 + Orange 玻璃拟态风格 |
| Markdown 编辑器 | Tiptap + tiptap-markdown |
| 源码编辑器 | CodeMirror 6 |
| 桌面壳 | Tauri 2（In-Process 单进程底座已落地） |

## 📌 依赖版本治理策略

- 所有新增依赖必须使用**精确版本**（Cargo 用 `=x.y.z`，npm 用 `x.y.z`）。
- 当前全仓版本台账见 `memories/PACKAGE_VERSIONS.md`（含根项目、UI、E2E、主题测试子工程与 Rust 可选/开发依赖）。
- 升级依赖时，必须同步更新对应 `package.json` / `Cargo.toml` 与版本台账文档，避免隐式漂移。

## 🚀 快速开始

### 环境要求

- Rust 1.75+（`rustup default stable`）
- Node.js 18+（修改后台 UI 时需要）
- cargo-watch（开发模式热重载，`cargo install cargo-watch`）

### 编译运行

```bash
# 克隆项目
git clone https://github.com/wenaryHY/inkforge.git
cd inkforge

# 安装前端依赖
cd src/admin/ui && npm install && cd ../../..

# 编译（首次需要下载依赖，约 2~5 分钟）
cargo build --release

# 运行
cargo run --release
```

默认情况下，后端会监听 `http://localhost:2000`。

### 开发模式

项目已配置 `concurrently` + `cargo-watch`，一条命令同时启动前后端：

```bash
npm run dev:watch
```

| 服务 | 地址 | 说明 |
|---|---|---|
| 管理后台 (Vite) | `http://localhost:5173/admin/` | 仅前端开发服务器（请勿作为联调主入口） |
| 前台 & API (Axum) | `http://localhost:2000` | 本地开发与联调唯一入口 |

Vite 开发服务器已配置代理，`/api`、`/ws`、`/uploads` 请求默认转发到 `2000` 端口的后端。

### 生产部署

```bash
# 构建前端
cd src/admin/ui && npm run build && cd ../../..

# 构建后端（前端产物会被嵌入）
cargo build --release
```

产物为单个二进制文件 `inkforge`，直接运行即可。

### Docker 部署

> 本地开发默认端口为 `2000`；当前 Docker 镜像默认通过环境变量将服务绑定到 `3000`，如有需要可自行覆盖。

```bash
docker build -t inkforge .
docker run -d \
  -p 3000:3000 \
  -e INKFORGE__RUNTIME__MODE=production \
  -e INKFORGE__AUTH__SECRET="$(openssl rand -hex 32)" \
  -v inkforge-uploads:/app/uploads \
  -v inkforge-backups:/app/backups \
  inkforge
```

镜像内置 Litestream，可按 `config/litestream.yml` 配置对象存储复制；**应用内备份模块的 S3 后端目前仍未真正接线**，如需对象存储同步请优先按 Litestream 链路配置。

裸机部署建议使用非 root systemd 服务、独立数据目录和最小发布包，详见 `docs/PRODUCTION_HARDENING.md`。

---

## 📁 项目结构

```text
inkforge/
├── src/
│   ├── main.rs              # 二进制入口（委托到 lib::serve）
│   ├── lib.rs               # 后端服务入口（供 Web / Tauri 复用）
│   ├── state.rs             # 全局状态
│   ├── ws.rs                # WebSocket 处理
│   ├── bootstrap/           # 配置加载 & 路由组装
│   ├── infra/               # 基础设施（错误处理等）
│   ├── modules/
│   │   ├── auth/            # 认证（handler → service → repository）
│   │   ├── setup/           # 安装向导
│   │   ├── post/            # 文章 / 页面
│   │   ├── comment/         # 评论
│   │   ├── plugin/          # 插件系统（trait、registry、manager、hooks、slots）
│   │   ├── category/        # 分类
│   │   ├── tag/             # 标签
│   │   ├── media/           # 媒体管理
│   │   ├── theme/           # 主题渲染
│   │   ├── seo/             # SEO（sitemap, robots, meta）
│   │   ├── setting/         # 系统设置
│   │   ├── backup/          # 备份恢复（含定时调度）
│   │   ├── trash/           # 统一回收站（含过期清理调度）
│   │   └── user/            # 用户管理
│   └── admin/ui/            # React 管理后台源码
├── migrations/              # SQLite 迁移文件（001–015）
├── config/                  # TOML 配置文件
├── plugins/                # 外部插件目录（build.rs 自动发现）
├── themes/default/          # 默认前台主题（MiniJinja 模板）
├── src-tauri/               # Tauri 桌面壳（In-Process 已落地）
├── uploads/                 # 上传文件目录
└── docker/                  # Docker 入口脚本
```

---

## ⚙️ 配置说明

通过 `config/default.toml` 或环境变量覆盖（前缀 `INKFORGE__`，双下划线分隔层级）：

```toml
[server]
host = "0.0.0.0"
port = 2000

[database]
url = "sqlite://inkforge.db?mode=rwc"

[auth]
secret = "inkforge-change-me-in-production"
expires_in_seconds = 604800

[storage]
upload_dir = "uploads"
max_upload_size_mb = 10

[theme]
theme_dir = "themes"
active_theme_fallback = "default"
default_mode = "system"

[paths]
admin_dist_dir = "src/admin/dist"
```

```bash
# 环境变量覆盖示例
export INKFORGE__SERVER__PORT=8080
export INKFORGE__AUTH__SECRET=your-production-secret
export INKFORGE__DATABASE__URL=sqlite:///data/inkforge.db?mode=rwc
```

---

## 🔮 路线图

- [x] WebSocket 实时评论通知
- [x] React 管理后台（Orange 玻璃拟态风格）
- [x] 页面双内容模型（Markdown + 自定义 HTML）
- [x] Tiptap 编辑器
- [x] 全文搜索（FTS5）
- [x] 统一回收站
- [x] SEO（Sitemap + OpenGraph + JSON-LD）
- [x] 备份与恢复（含定时调度）
- [x] API 版本化
- [x] Docker 部署 + Litestream
- [x] 安装向导（Web）
- [x] 插件系统（Rust trait：路由 / Hooks / 配置面板 / 前端插槽）
- [ ] 插件系统 WASM 运行时（热加载 / 沙箱 / 远程安装）
- [ ] S3/OSS 对象存储适配
- [ ] 多语言前台支持
- [x] Tauri In-Process 启动模型与单窗口状态路由
- [ ] Tauri 桌面端打包验收闭环

---

**License**: MIT
