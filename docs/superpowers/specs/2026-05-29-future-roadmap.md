# InkForge 未来方向记录

**日期:** 2026-05-29
**状态:** 待排期

---

## 已讨论但延后的功能

### 1. SSG 静态站点生成
- `inkforge build` → 纯静态 HTML
- MiniJinja 渲染引擎复用
- Zola 级速度（<1秒全站）
- 部署适配器（Cloudflare Pages / Vercel / Netlify）
- 图片优化管道（webp/avif、响应式 srcset）

### 2. CLI 脚手架工具
- `inkforge init` / `inkforge dev` / `inkforge plugin create` / `inkforge doctor`
- OpenAPI 文档自动生成 → TypeScript SDK

### 3. 内容协作工作流
- 草稿/版本历史/Diff 对比
- 审核流程、定时发布
- 多用户角色（Admin/Editor/Author）

### 4. AI 赋能（插件形态）
- AI 写作辅助、翻译、图片 Alt 文本
- 可选本地模型（llama.cpp）

### 5. 主题/插件 Registry
- 一键安装、版本管理、兼容性矩阵
- 等插件数量 > 20 后启动

### 6. Tauri 桌面打包
- 修复 .ico 打包问题
- `tauri build` 产出可分发的 exe

### 7. 主题构建链（Phase 5 原路线图）
- 主题输入/输出目录规范
- ink-data 数据注入协议
- 主题开发热重载
