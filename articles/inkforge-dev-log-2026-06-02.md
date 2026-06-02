# InkForge 开发日志：泛型、OOM 与换行符——一个无头 CMS 的五月生存报告

**2026-06-02 · 技术复盘**

---

## 开篇

这个月我们给 InkForge 加了五样东西，修了十二个以上的 bug，删了一千多行代码，炸了一次生产环境。写这篇日志的时候服务器还活着，先给自己鼓个掌 (￣▽￣)ノ

InkForge 是什么——一个 Rust + React 的无头 CMS，跑在 DigitalOcean 1C2G 的土豆服务器上。你问为什么是 1C2G？因为够用，而且穷。

---

## 1. 泛型抽象：151 处硬编码的末日

最开始的项目里，所有数据库操作都直接绑 `SqlitePool`。作者（我）当时的想法很朴素："反正就用 SQLite，写死怎么了？"

后来想支持 PostgreSQL，一数——20+ 个文件要改，151 处 `SqlitePool` 引用。那一刻我的表情：(╯°□°)╯︵ ┻━┻

于是花了两天做泛型抽象。所有数据访问函数签名从：

```rust
async fn get_post(pool: &SqlitePool, id: i64) -> Result<Post>
```

变成了：

```rust
async fn get_post<'e, E: sqlx::Executor<'e, Database = sqlx::Sqlite>>(
    executor: E,
    id: i64,
) -> Result<Post>
```

最终 17 个文件、112 个函数被修改。但好消息是：零成本抽象——编译后和硬编码生成的机器码一模一样，MIR 都没多一行。未来切 PostgreSQL 只需要改 4 个文件里的 `sqlx::Sqlite` → `sqlx::Postgres`。这才是 Rust 该有的样子 (•̀ᴗ•́)و

---

## 2. API Key 认证：无头 CMS 的入场券

无头 CMS 不提供 API Key 就像咖啡店不卖咖啡。加了一整套：

- Header 认证：`X-API-Key: ink_xxxx`
- SHA-256 哈希存储，只读权限，和现有 JWT 体系互补
- 后端中间件 + 前端管理界面 + Key 生成/吊销

这套没什么坑，属于照着文档写的标准操作。唯一的情绪波动是写前端 UI 时 Tailwind 的 flex 又没居中——日常。

---

## 3. Webhook 事件回调

利用已有的 `HookRegistry` 系统，在文章发布/更新时自动 POST JSON 到配置的 URL。附带：

- HMAC-SHA256 签名
- 失败自动重试
- 投递记录（时间、状态码、响应体）
- 前端 CRUD 管理界面

同样顺利。写到这里我已经开始飘了——"这个月效率真高啊"——然后认证系统的地雷就炸了。

---

## 4. 方案 C：统一认证的连环坑

一切从用户反馈开始："7 天免登录根本没用，半天就要重新登。"

排查过程就像剥洋葱，每剥一层都辣眼睛：

**第一层**：JWT 的 `exp` 写死 15 分钟，但 cookie 的 `Max-Age` 设了 7 天。token 过期了 cookie 还在——浏览器的 cookie 在说"我还能战"，服务端的 JWT 在说"你已经死了"。

**第二层**：refresh 接口只把新 token 放在 JSON body 里返回，不更新 cookie。前端刷新页面时内存里的 token 丢了，cookie 里的还是旧的，于是 401。

**第三层**：更离谱——前台（首页 / 文章页）和后台（admin 面板）用的是**两套完全不同的 token 体系**。前台的 token 叫 `site_token`，后台的叫 `admin_token`，两个中间件各自为政，互不认识。难怪用户在前台登录后进 admin 又要重新登。

最终方案我写了三行注释概括：

```
方案 A: 拆分前台/后台 cookie，互不干扰 → 前台不能复用后台认证
方案 B: 完全统一成一棵 cookie → 破坏现有路由隔离
方案 C: JWT 跟随 cookie Max-Age，refresh 同时 Set-Cookie，
       api.ts 加 cookie 兜底（内存 token 丢了就用 cookie），
       前台统一到同一套认证体系 ← 选这个
```

实现过程中的附加伤害：
- Setup 流程（首次安装）缺 refresh token 生成逻辑，管理员 15 分钟准时被踢
- Register 接口不支持 `remember_me` 参数，注册完就得手动再登一次
- FAB 按钮（浮动操作按钮）在刷新瞬间闪现到 `(0, 0)` 坐标再跳回来——React 水合时序问题

两次完整的认证体系审计，修了 12+ 个相关问题。当时的心理状态：(╬ Ò﹏Ó)

---

## 5. 类型和命名：趁乱推行的规范

趁着大规模重构，顺手推了几条命名规范：

- 所有时长相关类型从 `i64` 改成 `u64`——时间不长负数，让类型系统替你挡 bug
- Cookie 名称提取为常量：`SESSION_COOKIE_NAME_FOR_JWT_ACCESS_TOKEN`，长是长了点，但搜代码时一搜一个准
- 命名优先级：正确性 >> 准确性 >> 统一性。不接受为了"统一风格"而用模糊的名字

这部分是代码洁癖的快乐时光 ٩(ˊᗜˋ*)و

---

## 6. 缩略图系统：1113 行代码的墓碑

这是本月最痛苦的一段。我自信满满地开始做媒体处理：

- 用 Rust 的 `image` crate 做 resize + WebP 编码
- TDD 流程：先写 3 个测试，RED → GREEN → REFACTOR
- 测试全绿。心想："成了！"

然后上传了一张 4000×3000 的 iPhone 照片——进程被 Linux OOM Killer 直接 SIGKILL。

于是开始叠防御：

**第一层**：前端文件大小限制（10MB）→ 一个 4000×3000 JPEG 只有 3MB，轻松绕过。

**第二层**：后端文件大小检查 → OOM 发生在 decode 后、resize 时，不是上传时。

**第三层**：`std::panic::catch_unwind` 兜底 → `image` crate 的 `resize_exact` 是纯内存分配失败，不是 panic，catch 不到。

**第四层**：解码前按像素数估算内存（`宽 × 高 × 4 字节 × 3 次 resize`）→ 终于能提前拒绝了。

但故事没完——1C2G 服务器上 4000×3000 解码后就是 48MB 位图，resize 三次就是三次分配。加上系统开销，OOM 只是时间问题。

于是改成异步 worker + 任务表 + 重试：上传 → 入队 → worker 处理 → 更新任务状态。还是 OOM。

最终在 GitHub 上搜到 `image` crate 的 **[issue #2340](https://github.com/image-rs/image/issues/2340)**：`resize_exact` 内存分配失败是已知 bug，官方标记 "help wanted"——没修。

那天晚上我干了这件事：

```bash
git rm -r src/media/thumbnail/
# deleted: 1113 lines
```

缩略图功能回退。结论：无头 CMS 的图片优化应该丢给前端框架——Next.js Image、Nuxt Image 做得比一个 Rust 后端好得多。硬要在 1C2G 服务器上做服务端图片处理，是架构上的傲慢 (´-﹏-`；)

---

## 7. 部署灾难：当 CRLF 遇上 LF

修完所有代码，打包部署。WSL 里编译的二进制推到服务器，SQLx 迁移检查失败：

```
migration checksum mismatch
```

查了半天——是 SQL 迁移文件的换行符问题。Windows/WSL 下 Git 默认把 `.sql` 文件 checkout 成 CRLF，而服务器上之前生成 migration hash 时用的是 LF。同一个文件，不同的 hash。

修理过程：

1. Python 脚本修 hash → 单引号被 PowerShell 当成命令的一部分吃掉了
2. 直接删 `_sqlx_migrations` 表 → 删完只剩 1 行 → 全量 re-apply → FTS5（全文搜索）建索引报错
3. 从备份恢复数据库 → 文件权限是 root → inkforge 用户写不进去
4. `chown inkforge:inkforge` → 终于跑起来了

整整 30 分钟的生产宕机。根因：**换行符**。

最后的 `deploy.sh` 第一行永远是这个：

```bash
dos2unix migrations/*.sql
```

然后才编译、SCP 上传。一劳永逸 (눈_눈)

---

## 总结

这个月写了很多代码，删了更多代码，炸了一次生产。但回头看，每个坑都在教一件事：

- **泛型抽象**早点做——重构 151 处的成本和一开始就写泛型差不了太多，但积压越久越不敢碰
- **不要假设库的实现是可靠的**——`image` crate 的 OOM bug 让我明白，TDD 只能保证你的逻辑对，保证不了依赖没 bug
- **服务器不是本地**——CRLF 这种问题在本地开发永远遇不到，但它会在你最不想它出现的时候出现
- **有些事不该你做**——无头 CMS 做图片优化就是越界了，把职责交还给正确的工具

写这篇日志的时候，InkForge 安静地跑在土豆服务器上，CPU 占用 3%。前面的路还很长，但至少这个月活下来了。

下个月见 (๑•̀ㅂ•́)و✧
