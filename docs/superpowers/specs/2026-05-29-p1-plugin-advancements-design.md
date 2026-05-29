# 插件 P1 进阶能力 — 设计文档

**日期:** 2026-05-29
**状态:** 已确认
**依赖:** Phase 4a（manifest 发现已完成）

---

## 已确认决策总览

### 1. 分次迭代
- P1a: Hooks 系统（首轮）
- P1b: 配置面板（次轮）
- P1c: 前端插槽（末轮）

### 2. Hooks 模型
- Directus 式 Filter + Action 二分
- Filter: 修改数据（串行管道）
- Action: 触发副作用（并行 + 5s 超时）

### 3. 首轮 5 个钩子

| 钩子 | 类型 | 调用策略 | 错误处理 |
|------|------|----------|----------|
| `post.before_save` | Filter | 串行管道 | 失败回滚 500 |
| `post.after_publish` | Action | 并行+5s超时 | 吞错误记日志 |
| `post.after_save` | Action | 并行+5s超时 | 吞错误记日志 |
| `post.before_render` | Filter | 串行管道 | 跳过该插件继续 |
| `comment.before_create` | Filter | 串行管道 | 拒绝 400+原因 |

### 4. Hooks 技术设计

| 维度 | 决定 |
|------|------|
| Handler 类型 | `#[async_trait]` trait（FilterHook / ActionHook），不用 Box<dyn Fn> |
| Filter 数据传递 | 通过 `&mut HookContext` 修改，不返回到值，签名统一 |
| Action 调用策略 | `tokio::spawn` 并行 + 5s timeout |
| 字段权限 | `PostFields`（可写）/ `PostMeta`（只读），类型系统约束，不做运行时权限 |
| 优先级 | priority 0-20（默认10）+ 插件名字典序 tie-break |
| Trait 集成 | `Plugin` trait 新增 `fn hooks(&self) -> Vec<Hook>`，默认空 |
| HookRegistry | `Arc<RwLock<HashMap<String, Vec<RegisteredHook>>>>` 按钩子名索引 |
| 卸载 | `unregister_all(plugin_name)` 原子移除 |

### 5. 否决项
- `template.before_render` 被否决（绑定 MiniJinja）
- 改为 `post.before_render`，插件返回数据合并到模板 Context 的 `plugins.{plugin_name}.xxx` 命名空间

### 6. HookContext 字段（每种钩子不同）

| 钩子 | HookContext 包含 |
|------|-----------------|
| `post.before_save` | `post_fields: &mut PostFields`, `post_meta: &PostMeta`, `request_ip`, `user_agent` |
| `post.after_publish` | `post: &Post`, `old_status`, `new_status` |
| `post.after_save` | `post: &Post`, `is_new` |
| `post.before_render` | `post: &Post`, `extra_context: &mut HashMap<String, Value>` |
| `comment.before_create` | `comment_fields: &mut CommentFields`, `parent_post: &Post`, `request_ip` |

### 7. 配置面板

| 插件类型 | manifest | 前端 |
|----------|----------|------|
| 纯后端 | 无 `[admin]` | 无管理界面 |
| 后端+前端 | `[admin] enabled=true, entry="settings.html"` | iframe 加载独立 HTML |

简单配置项 manifest 声明式（text/textarea/bool/select/number），select 需 options：
```toml
[[settings]]
key = "theme"
type = "select"
default = "light"
options = [
    { value = "light", label = "浅色" },
    { value = "dark", label = "深色" },
]
```

存储：`plugin_settings` 表（plugin_name, key, value, updated_at）。

统一 resources 根路径：
```toml
[resources]
admin_root = "admin/"
```

### 8. 前端插槽（6 个注入点）

| slot ID | 位置 |
|---------|------|
| `dashboard.widget` | 仪表盘首页卡片区 |
| `post_editor.sidebar` | 文章编辑器右侧面板 |
| `sidebar.menu_item` | 侧边栏新增菜单项 |
| `settings.sub_section` | 系统设置页 tab |
| `post_list.action_bar` | 文章列表工具栏 |
| `login.form_below` | 登录表单下方 |

iframe postMessage 通信协议（按审查加了安全握手）：

1. 宿主发 `{ type: "init", token: "<uuid>" }` 给 iframe
2. 插件所有后续消息必须带此 token，宿主校验
3. 插件→宿主: `{ type: "resize", token, height }` / `{ type: "navigate", token, path }`
4. 宿主→插件: `{ type: "context", token, data: { post_id, user, lang } }`
5. 宿主校验 `event.origin`
6. 卸载: 宿主发 `{ type: "host_unload", token }` → iframe 1s 内回复 → 移除 DOM

### 9. Hook 执行与事务边界
- `before_save` 与 service 层事务同生命周期
- HookContext 携带 `tx: &TransactionHandle`（如果钩子需要读 DB 验证数据）

### 10. 插件间依赖
- 首轮不实现 `depends_on`
- 仅通过 priority + 插件名字典序控制执行序

---

**全部 10 项设计已闭环。首轮 P1a (Hooks) 进入实施计划。**
