# 投递材料：AI 探索助理（实习）— 广州向日葵信息科技

## 项目简述（简历用）

**InkForge（墨炉）** — 个人全栈项目。用 Rust（Axum + SQLite + MiniJinja）+ React 19 从零构建的 CMS，支持文章/页面双内容模型、Tiptap 双模式编辑器、插件系统（路由/模板/Hooks/配置面板）、OAuth2 刷新令牌、Tauri 桌面壳。部署在 wenary.me，GitHub 开源。

GitHub: https://github.com/wenaryHY/inkforge
线上: https://wenary.me

---

## Q1: 你折腾过的一个最有意思的技术项目

InkForge——一个从零用 Rust 构建的完整 CMS 平台。从架构设计到安全上线，全程一个人完成。核心实现包括：

- Tiptap + CodeMirror 双模式编辑器，所见即所得与源码自由切换
- MiniJinja 服务端模板渲染，支持主题 ZIP 上传与切换
- 插件系统六层架构：trait → manifest 自动发现 → 启用/禁用 → Hooks（Filter/Action）→ 配置面板 → 前端插槽
- OAuth2 风格认证：access token 15min + refresh token 7d HttpOnly cookie + token rotation 防并发
- SQLite WAL 模式优化 + MiniJinja Environment 缓存，TTFB < 50ms
- Tauri 2 In-Process 桌面壳

整个开发过程深度使用 AI agent 辅助——不只是聊天对话，而是用 agent 做架构讨论、代码审查、安全测试和 bug 调试。项目目前部署运行在 wenary.me。

## Q2: 你目前如何使用 AI（具体场景）

我不只是和 AI 对话——我用 AI agent 做五种具体的事：

1. **架构设计**：新功能启动前，先用 brainstorming 流程和 agent 多轮讨论方案，确认后再写代码。比如插件系统设计时，agent 先调研了 Halo、WordPress、Jenkins、VSCode 四个成熟系统的插件机制，再对比提出适合 InkForge 的方案
2. **安全审计**：用 agent 调度多个 sec-auditor 子 agent，对我自己的 CMS 跑了全套测试——nuclei（6220 模板）、sqlmap、XSStrike、DalFox、TruffleHog（7248 文件块）、Gitleaks 等 10+ 工具，从代码到线上闭环验证
3. **代码审查**：写完代码后派 code-reviewer agent 做两轮 review——先是 spec compliance（是否按设计实现），再是 code quality（代码质量），不通过打回去重写
4. **Bug 定位**：遇到 Tiptap Color 扩展颜色不渲染的问题，让 agent 去 GitHub issues 和官方文档交叉搜索，最终发现是 Color 从错误包导入导致的
5. **环境配置**：让 agent 通过 WSL 安装 hackingtool 安全工具集，一步步排查权限、路径、依赖问题

## Q3: 过去 6 个月里，你用 AI 工具亲自解决过的一个最棘手的问题

最棘手的是 InkForge 自定义缓存导致的竞态 bug。

我写了一个 SWR 缓存（stale-while-revalidate），但 mutation 后缓存失效不彻底——保存设置后列表仍显示旧数据，必须手动刷新浏览器才能看到更新。我尝试加了两轮版本号机制：第一轮在缓存写入时比较版本号，第二轮在失效时标记"飞行中的请求"。都没修好。

最后让 AI agent 去 Stack Overflow 和 TkDodo（TanStack Query 维护者）的博客调研。agent 分析后指出：缓存失效是计算机科学最难的问题之一，自己写的 SWR 要处理飞行请求、版本竞争、前缀匹配、过期策略等边界条件，不如用社区验证的方案。推荐迁移到 TanStack Query（React Query）——它的 `invalidateQueries()` 机制经过 10 年社区迭代。

迁移后：16 个页面组件全部改用 `useQuery`/`useMutation`，所有写操作自动调 `invalidateQueries`，数据不一致问题彻底解决。而且代码量反而减少了——删了 ~130 行自定义缓存代码，换成标准 API。

这个经历让我深刻理解了"不自己造轮子"的道理——尤其是涉及分布式一致性的领域，社区的方案比你想象的可靠得多。

## 为什么想来？（300 字以内）

我觉得这份岗位描述在直接对我说话。

你们说"不只会谈热爱，而是真的做过东西、踩过坑"——我现在就在做这件事：一个人从零写了个 Rust CMS，为了修一个颜色渲染 bug 查了三轮源码才发现是 ammonia builder 被写成了死代码；为了优化保存 8 秒延迟去开 SQLite WAL、改批量 API、修 std::sync::RwLock；为了编辑器好用，装 6 个 Tiptap 扩展、手写 120 行 CSS。每个功能都踩过坑，踩完记录下来，下次就能避。

我来，是因为我想把这种"用 AI 加速折腾"的能力，从个人项目带进真实的业务场景。我有 GitHub 项目、有踩坑经历、有和 AI agent 深度协作的实际经验——我不需要别人教我"AI 怎么用"，我可以帮你们一起找答案。

## 附件链接

- GitHub: https://github.com/wenaryHY/inkforge
- 线上站点: https://wenary.me
