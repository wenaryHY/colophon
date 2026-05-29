# P1c — 前端插槽 Implementation Plan

**状态:** 🔲 待实施

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现插件前端插槽系统：6 个 injection point（`dashboard.widget`、`post_editor.sidebar`、`sidebar.menu_item`、`settings.sub_section`、`post_list.action_bar`、`login.form_below`）、iframe postMessage 安全握手协议、后端 slots API、前端 SlotContainer + SlotsContext + SlotRenderer。

**Architecture:**

```
App.tsx 加载时调用 GET /api/v1/admin/plugins/slots
  → 返回 [{ target, label, entry, width, height, plugin_name, plugin_title, iframe_url }]
  → 存入 SlotsContext

各页面组件:
  <SlotRenderer target="post_editor.sidebar" context={{ post_id }} />
    → useSlots() 过滤所有匹配 target 的 slot
    → 为每个 slot 渲染 <SlotContainer>
      → iframe src = iframe_url
      → postMessage 安全握手:
         1. onLoad → 宿主发 { type:"init", token }
         2. 插件后续消息必须带此 token
         3. 宿主→插件: { type:"context", token, data }
         4. 插件→宿主: { type:"resize", token, height } / { type:"navigate", token, path }
         5. 宿主校验 event.origin
         6. 卸载: { type:"host_unload", token } → 等待 1s → 移除 DOM
```

**Tech Stack:** Rust, serde_json, axum, React 19, TypeScript, react-router-dom, postMessage API

**Pre-requisites:** Phase 1–4a + P1a Hooks + P1b 配置面板已完成，`cargo test -p inkforge` 全绿。manifest.rs 中 `SlotDef` 结构体已预留。

**运行测试命令:** `cargo test -p inkforge plugin_slots && npm --prefix src/admin/ui run build`

---

## 文件结构

| 文件 | 职责 | 操作 |
|------|------|------|
| `src/modules/plugin/handler.rs` | 添加 list_slots handler | 修改 |
| `src/bootstrap/router.rs` | 添加 GET /api/v1/admin/plugins/slots | 修改 |
| `src/admin/ui/src/components/SlotContainer.tsx` | iframe 包装器 + postMessage 安全握手 | 新建 |
| `src/admin/ui/src/lib/slots.ts` | 类型定义 + fetchSlots + SlotsContext + SlotRenderer | 新建 |
| `src/admin/ui/src/App.tsx` | 集成 SlotsProvider + 初始化加载 | 修改 |
| `src/admin/ui/src/pages/Posts.tsx` | 集成 dashboard.widget + post_list.action_bar | 修改 |
| `src/admin/ui/src/pages/PostEditor.tsx` | 集成 post_editor.sidebar | 修改 |
| `src/admin/ui/src/pages/Settings.tsx` | 集成 settings.sub_section | 修改 |
| `src/admin/ui/src/pages/Login.tsx` | 集成 login.form_below | 修改 |
| `src/admin/ui/src/components/Sidebar.tsx` | 集成 sidebar.menu_item | 修改 |

---

## Task 1: 后端 slots API — handler + 路由

**Files:**
- Modify: `src/modules/plugin/handler.rs`
- Modify: `src/bootstrap/router.rs`

**目的:** 添加 `GET /api/v1/admin/plugins/slots` 端点，遍历所有已发现插件的 manifest，提取 slots 声明并构造 iframe URL。

### Step 1: 添加 list_slots handler

在 `src/modules/plugin/handler.rs` 末尾追加：

```rust
pub async fn list_slots(
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    let slot_infos: Vec<serde_json::Value> = state
        .plugin_manager
        .discovered_manifests()
        .into_iter()
        .filter_map(|m| {
            let slots = m.slots?;
            let admin_root = m
                .resources
                .as_ref()
                .and_then(|r| r.admin_root.as_deref())
                .unwrap_or("admin/");
            let plugin_id = m.plugin.id.clone();
            let plugin_title = m.plugin.title.clone();
            Some(
                slots
                    .into_iter()
                    .map(move |s| {
                        let base = admin_root.trim_end_matches('/');
                        let entry_path = s.entry.trim_start_matches('/');
                        let iframe_url = format!(
                            "/static/plugins/{}/{}/{}",
                            plugin_id, base, entry_path
                        );
                        serde_json::json!({
                            "target": s.target,
                            "label": s.label,
                            "entry": s.entry,
                            "width": s.width,
                            "height": s.height,
                            "plugin_name": plugin_id,
                            "plugin_title": plugin_title,
                            "iframe_url": iframe_url,
                        })
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .flatten()
        .collect();

    Ok(Json(ApiResponse::success(serde_json::json!({
        "slots": slot_infos,
    }))))
}
```

### Step 2: 添加路由

在 `src/bootstrap/router.rs` 的 v1 router 中，插件路由区域（`/api/v1/admin/plugins/:name/settings` 附近）添加：

```rust
.route("/api/v1/admin/plugins/slots",
    get(crate::modules::plugin::handler::list_slots))
```

### Step 3: 编译验证

```bash
cargo check -p inkforge
```

### Step 4: Commit

```bash
git add src/modules/plugin/handler.rs src/bootstrap/router.rs
git commit -m "feat: 添加 GET /api/v1/admin/plugins/slots 端点，返回所有插件 slots 声明"
```

---

## Task 2: SlotContainer 组件

**Files:**
- Create: `src/admin/ui/src/components/SlotContainer.tsx`

**目的:** 创建 iframe 包装器，实现完整的 postMessage 安全握手协议。

```tsx
import { useCallback, useEffect, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import type { SlotInfo } from '../lib/slots';

interface SlotContainerProps {
  slot: SlotInfo;
  context?: Record<string, unknown>;
}

function generateToken(): string {
  return crypto.randomUUID();
}

export default function SlotContainer({ slot, context }: SlotContainerProps) {
  const iframeRef = useRef<HTMLIFrameElement>(null);
  const tokenRef = useRef<string>(generateToken());
  const navigate = useNavigate();
  const [iframeHeight, setIframeHeight] = useState<number>(slot.height ?? 200);

  const sendToIframe = useCallback(
    (message: Record<string, unknown>) => {
      iframeRef.current?.contentWindow?.postMessage(message, window.location.origin);
    },
    [],
  );

  const handleMessage = useCallback(
    (event: MessageEvent) => {
      if (event.origin !== window.location.origin) {
        return;
      }
      const data = event.data;
      if (!data || typeof data !== 'object' || data.token !== tokenRef.current) {
        return;
      }
      switch (data.type) {
        case 'resize': {
          const h = typeof data.height === 'number' ? data.height : 0;
          if (h > 0) {
            setIframeHeight(h);
          }
          break;
        }
        case 'navigate': {
          const path = typeof data.path === 'string' ? data.path : '';
          if (path) {
            navigate(path);
          }
          break;
        }
      }
    },
    [navigate],
  );

  useEffect(() => {
    window.addEventListener('message', handleMessage);
    return () => {
      window.removeEventListener('message', handleMessage);
    };
  }, [handleMessage]);

  const handleIframeLoad = useCallback(() => {
    sendToIframe({ type: 'init', token: tokenRef.current });
    if (context) {
      sendToIframe({ type: 'context', token: tokenRef.current, data: context });
    }
  }, [sendToIframe, context]);

  useEffect(() => {
    return () => {
      const msg = { type: 'host_unload', token: tokenRef.current };
      iframeRef.current?.contentWindow?.postMessage(msg, window.location.origin);
    };
  }, []);

  return (
    <iframe
      ref={iframeRef}
      src={slot.iframe_url}
      title={`plugin-slot-${slot.plugin_name}-${slot.target}`}
      sandbox="allow-scripts allow-same-origin"
      onLoad={handleIframeLoad}
      style={{
        width: slot.width ? `${slot.width}px` : '100%',
        height: `${iframeHeight}px`,
        border: 'none',
        display: 'block',
      }}
    />
  );
}
```

### Commit

```bash
git add src/admin/ui/src/components/SlotContainer.tsx
git commit -m "feat: 创建 SlotContainer 组件，实现 postMessage 安全握手协议"
```

---

## Task 3: 前端 slots API + SlotsContext + SlotRenderer

**Files:**
- Create: `src/admin/ui/src/lib/slots.ts`

**目的:** 定义 SlotInfo 类型、fetchSlots 函数、SlotsProvider context、useSlots hook、SlotRenderer 组件。

```tsx
import { createContext, useCallback, useContext, useEffect, useState } from 'react';
import { API_PREFIX, apiData } from './api';
import SlotContainer from '../components/SlotContainer';

export interface SlotInfo {
  target: string;
  label: string;
  entry: string;
  width: number | null;
  height: number | null;
  plugin_name: string;
  plugin_title: string;
  iframe_url: string;
}

interface SlotsResponse {
  slots: SlotInfo[];
}

interface SlotsContextValue {
  slots: SlotInfo[];
  loading: boolean;
  refresh: () => Promise<void>;
}

const SlotsContext = createContext<SlotsContextValue>({
  slots: [],
  loading: false,
  refresh: async () => {},
});

export function useSlots() {
  return useContext(SlotsContext);
}

export function SlotsProvider({ children }: { children: React.ReactNode }) {
  const [slots, setSlots] = useState<SlotInfo[]>([]);
  const [loading, setLoading] = useState(true);

  const fetchSlots = useCallback(async () => {
    try {
      const data = await apiData<SlotsResponse>(`${API_PREFIX}/admin/plugins/slots`);
      setSlots(data.slots || []);
    } catch {
      setSlots([]);
    } finally {
      setLoading(false);
    }
  }, []);

  const refresh = useCallback(async () => {
    setLoading(true);
    await fetchSlots();
  }, [fetchSlots]);

  useEffect(() => {
    void fetchSlots();
  }, [fetchSlots]);

  return (
    <SlotsContext.Provider value={{ slots, loading, refresh }}>
      {children}
    </SlotsContext.Provider>
  );
}

export function SlotRenderer({
  target,
  context,
}: {
  target: string;
  context?: Record<string, unknown>;
}) {
  const { slots } = useSlots();
  const matched = slots.filter((s) => s.target === target);
  if (matched.length === 0) {
    return null;
  }
  return (
    <>
      {matched.map((s) => (
        <SlotContainer
          key={`${s.plugin_name}-${s.target}`}
          slot={s}
          context={context}
        />
      ))}
    </>
  );
}
```

### Commit

```bash
git add src/admin/ui/src/lib/slots.ts
git commit -m "feat: 创建 slots API 模块，含 SlotInfo 类型、SlotsProvider、SlotRenderer"
```

---

## Task 4: App.tsx 集成 SlotsProvider

**Files:**
- Modify: `src/admin/ui/src/App.tsx`

**目的:** 在 App 根组件中包裹 SlotsProvider，以便所有子页面都能通过 useSlots() 获取插槽数据。

### Step 1: 添加 import

在 `src/admin/ui/src/App.tsx` 顶部 import 区添加：

```tsx
import { SlotsProvider } from './lib/slots';
```

### Step 2: 包裹 AdminGate

修改 `AdminGate` 的渲染部分，用 `SlotsProvider` 包裹：

找到 `if (!token) { return <Login />; }` 和 `return <AdminLayout />;`，替换为：

```tsx
  if (!token) {
    return (
      <SlotsProvider>
        <Login />
      </SlotsProvider>
    );
  }

  return (
    <SlotsProvider>
      <AdminLayout />
    </SlotsProvider>
  );
```

注意：Login 页面不在 AdminGate 的 token 检查内也会用到 SlotsContext（login.form_below），所以需要包住。实际操作：在 `export default function App()` 的 `<BrowserRouter>` 内用 `<SlotsProvider>` 包裹 `<Routes>`，这样所有路由都共享同一个 SlotsContext。

更好的方案是直接在 `App` 组件中包裹 `SlotsProvider`：

找到 `export default function App() {` 及其返回的 JSX，将 `<BrowserRouter>` 内的内容包裹：

```tsx
export default function App() {
  return (
    <BrowserRouter basename="/admin">
      <SlotsProvider>
        <Routes>
          ...
        </Routes>
      </SlotsProvider>
    </BrowserRouter>
  );
}
```

### Step 3: 编译验证

```bash
npm --prefix src/admin/ui run build
```

### Step 4: Commit

```bash
git add src/admin/ui/src/App.tsx
git commit -m "feat: App.tsx 集成 SlotsProvider，全局提供插槽上下文"
```

---

## Task 5: Posts.tsx 集成两个 slot 注入点

**Files:**
- Modify: `src/admin/ui/src/pages/Posts.tsx`

**目的:** 在文章列表页集成 `dashboard.widget`（统计卡片下方）和 `post_list.action_bar`（PageHeader 操作区旁边）。

### Step 1: 添加 import

在 `src/admin/ui/src/pages/Posts.tsx` 顶部 import 区添加：

```tsx
import { SlotRenderer } from '../lib/slots';
```

### Step 2: 添加 dashboard.widget（统计卡片下方）

在统计卡片 `</div>` 和 `<PageHeader` 之间插入：

```tsx
      <SlotRenderer target="dashboard.widget" />
```

### Step 3: 添加 post_list.action_bar（PageHeader 操作区旁边）

找到 `<PageHeader` 组件调用（约第 219-223 行），在 `actions={...}` 的末尾插入 slot renderer。由于 PageHeader 的 actions 是单个 ReactNode，需要包装：

将：
```tsx
      <PageHeader
        title={t('postsTitle')}
        subtitle={format('postsCount', { count: total })}
        actions={<Button onClick={() => navigate(contentTypeTab === 'post' ? '/posts/new' : '/posts/new?type=page')}><IconPlus /> {contentTypeTab === 'post' ? t('newPost') : t('newPage', '新建页面')}</Button>}
      />
```

替换为：
```tsx
      <PageHeader
        title={t('postsTitle')}
        subtitle={format('postsCount', { count: total })}
        actions={
          <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
            <SlotRenderer target="post_list.action_bar" />
            <Button onClick={() => navigate(contentTypeTab === 'post' ? '/posts/new' : '/posts/new?type=page')}><IconPlus /> {contentTypeTab === 'post' ? t('newPost') : t('newPage', '新建页面')}</Button>
          </div>
        }
      />
```

### Step 4: 编译验证

```bash
npm --prefix src/admin/ui run build
```

### Step 5: Commit

```bash
git add src/admin/ui/src/pages/Posts.tsx
git commit -m "feat: Posts.tsx 集成 dashboard.widget 和 post_list.action_bar 插槽"
```

---

## Task 6: PostEditor.tsx 集成 post_editor.sidebar

**Files:**
- Modify: `src/admin/ui/src/pages/PostEditor.tsx`

**目的:** 在文章编辑器右侧面板（发布设置）下方插入插件插槽。

### Step 1: 添加 import

在 `src/admin/ui/src/pages/PostEditor.tsx` 顶部 import 区添加：

```tsx
import { SlotRenderer } from '../lib/slots';
```

### Step 2: 在右侧面板底部添加插槽

找到右侧面板的最后一个 `</div>`（分类和标签面板之后，约第 483 行），在 `{/* 右侧：发布设置 */}` 这个大 div 的末尾（即 `</div>` 关闭之前）添加插槽：

在第 483 行 `</div>`（分类和标签面板）之后、第 484 行之前（右侧面板整体闭合前）插入：

```tsx
          <SlotRenderer
            target="post_editor.sidebar"
            context={post ? { post_id: post.id } : undefined}
          />
```

具体位置：在分类和标签面板的闭合 `</div>` 之后，右侧面板最外层大 div 闭合之前。

### Step 3: 获取 post 引用

当前代码中 `post` 状态在 PostEditor 组件作用域内可用，无需额外操作。`context` 将在 post 加载后传入 slot。

### Step 4: 编译验证

```bash
npm --prefix src/admin/ui run build
```

### Step 5: Commit

```bash
git add src/admin/ui/src/pages/PostEditor.tsx
git commit -m "feat: PostEditor.tsx 集成 post_editor.sidebar 插槽"
```

---

## Task 7: Settings.tsx 集成 settings.sub_section

**Files:**
- Modify: `src/admin/ui/src/pages/Settings.tsx`

**目的:** 在系统设置页插入插件提供的子设置区块。

### Step 1: 添加 import

在 `src/admin/ui/src/pages/Settings.tsx` 顶部 import 区添加：

```tsx
import { SlotRenderer } from '../lib/slots';
```

### Step 2: 在设置页末尾添加插槽

找到所有 `<SettingSection>` 调用之后（约第 561 行，`</>` 返回之前），在界面设置 section 之后插入：

在第 416 行（界面设置 section 闭合）和 第 418 行（数据备份 section 开始）之间，或者在所有 SettingSection 之后、最终闭合前插入：

```tsx
      <SlotRenderer target="settings.sub_section" />
```

建议插入在界面设置 section（约第 416 行闭合）之后、数据备份 section 之前：

```tsx
      </SettingSection>

      <SlotRenderer target="settings.sub_section" />

      <SettingSection
        title={t('dataBackup')}
```

### Step 3: 编译验证

```bash
npm --prefix src/admin/ui run build
```

### Step 4: Commit

```bash
git add src/admin/ui/src/pages/Settings.tsx
git commit -m "feat: Settings.tsx 集成 settings.sub_section 插槽"
```

---

## Task 8: Login.tsx 集成 login.form_below

**Files:**
- Modify: `src/admin/ui/src/pages/Login.tsx`

**目的:** 在登录表单下方插入插件提供的自定义区域。

### Step 1: 添加 import

在 `src/admin/ui/src/pages/Login.tsx` 顶部 import 区添加：

```tsx
import { SlotRenderer } from '../lib/slots';
```

### Step 2: 在登录卡片底部添加插槽

在登录卡片内，底部链接之前（约第 369 行 `{/* 底部链接 */}` 之前），插入 slot 渲染：

在表单区域闭合 `</div>` 之后（约第 367 行）和 `{/* 底部链接 */}` 注释（约第 370 行）之间插入：

```tsx

        <SlotRenderer target="login.form_below" />
```

### Step 3: 编译验证

```bash
npm --prefix src/admin/ui run build
```

### Step 4: Commit

```bash
git add src/admin/ui/src/pages/Login.tsx
git commit -m "feat: Login.tsx 集成 login.form_below 插槽"
```

---

## Task 9: Sidebar.tsx 集成 sidebar.menu_item

**Files:**
- Modify: `src/admin/ui/src/components/Sidebar.tsx`

**目的:** 在侧边栏的系统导航分组下方插入插件提供的菜单项。

### Step 1: 添加 import

在 `src/admin/ui/src/components/Sidebar.tsx` 顶部 import 区添加：

```tsx
import { SlotRenderer } from '../lib/slots';
```

### Step 2: 在导航区底部添加插槽

在侧边栏的 `<nav>` 区域内、所有 navConfig group 的 map 渲染之后（约第 189 行)，`</nav>` 闭合之前插入：

在第 189 行 `))}`（navConfig.map 结束）和第 190 行 `</nav>` 之间插入：

```tsx
        <div style={{
          padding: '16px 12px 8px',
          fontSize: '11px',
          fontWeight: 700,
          color: 'var(--md-on-surface-variant)',
          textTransform: 'uppercase',
          letterSpacing: '0.1em',
        }}>
          {t('plugins')}
        </div>
        <SlotRenderer target="sidebar.menu_item" />
```

注意：需要在 i18n 中添加 `plugins` 翻译键。如果暂时不想修改 i18n 文件，可用硬编码字符串代替 `{t('plugins')}`。

### Step 3: 编译验证

```bash
npm --prefix src/admin/ui run build
```

### Step 4: Commit

```bash
git add src/admin/ui/src/components/Sidebar.tsx
git commit -m "feat: Sidebar.tsx 集成 sidebar.menu_item 插槽"
```

---

## Task 10: 全量编译与验证

**Files:**
- （所有已修改文件）

**目的:** 确保前后端编译通过，运行时无报错。

### Step 1: 后端编译检查

```bash
cargo check -p inkforge
```

Expected: 编译通过，无 warning。handler.rs 的 list_slots 函数签名匹配 axum handler 要求。

### Step 2: 前端编译检查

```bash
npm --prefix src/admin/ui run build
```

Expected: TypeScript 编译通过，Vite 构建成功。

### Step 3: 代码检查 — 确保所有 import 路径正确

验证清单：
- `src/admin/ui/src/App.tsx` → `import { SlotsProvider } from './lib/slots';` 路径正确（`App.tsx` 在 `src/`，`slots.ts` 在 `src/lib/`）
- `src/admin/ui/src/lib/slots.ts` → `import SlotContainer from '../components/SlotContainer';` 路径正确
- `src/admin/ui/src/lib/slots.ts` → `import { API_PREFIX, apiData } from './api';` 路径正确（同目录 `api.ts`）
- 各页面 → `import { SlotRenderer } from '../lib/slots';` 路径正确

### Step 4: 最小化插件示例验证（可选）

在 `plugins/hello-world-a3f9b2c1/plugin.toml` 中添加示例 slots：

```toml
[resources]
admin_root = "admin/"

[[slots]]
target = "dashboard.widget"
label = "Hello Widget"
entry = "widget.html"
width = 400
height = 250

[[slots]]
target = "sidebar.menu_item"
label = "Hello Menu"
entry = "menu.html"
width = null
height = 48
```

创建 `plugins/hello-world-a3f9b2c1/admin/widget.html` 和 `plugins/hello-world-a3f9b2c1/admin/menu.html` 作为测试 slot HTML。

### Step 5: 运行时验证

```bash
cargo run
# 访问 http://localhost:3000/admin
# 1. 检查 /api/v1/admin/plugins/slots 返回正确 JSON
# 2. 访问文章列表页，确认无控制台报错
# 3. 访问文章编辑器，确认无控制台报错
# 4. 访问设置页，确认无控制台报错
# 5. 访问登录页，确认无控制台报错
# 6. 确认侧边栏无控制台报错
```

Expected: 所有页面正常渲染，无 iframe 相关报错。如果没有插件注册 slot，SlotRenderer 返回 null 不渲染任何内容。

### Step 6: Commit

```bash
git add -A
git commit -m "feat: 完成 P1c 前端插槽系统 — 全部 10 个 Task 实施完毕"
```

---

## 附录 A: API 响应格式

`GET /api/v1/admin/plugins/slots` 响应示例：

```json
{
  "code": 0,
  "message": "success",
  "data": {
    "slots": [
      {
        "target": "dashboard.widget",
        "label": "Hello Widget",
        "entry": "widget.html",
        "width": 400,
        "height": 250,
        "plugin_name": "hello-world-a3f9b2c1",
        "plugin_title": "Hello World Plugin",
        "iframe_url": "/static/plugins/hello-world-a3f9b2c1/admin/widget.html"
      },
      {
        "target": "sidebar.menu_item",
        "label": "Hello Menu",
        "entry": "menu.html",
        "width": null,
        "height": 48,
        "plugin_name": "hello-world-a3f9b2c1",
        "plugin_title": "Hello World Plugin",
        "iframe_url": "/static/plugins/hello-world-a3f9b2c1/admin/menu.html"
      }
    ]
  }
}
```

## 附录 B: postMessage 通信协议

| 方向 | 消息格式 | 说明 |
|------|----------|------|
| 宿主→插件 | `{ type: "init", token: "<uuid>" }` | iframe onLoad 后立即发送 |
| 宿主→插件 | `{ type: "context", token: "<uuid>", data: {...} }` | 传递上下文（post_id、user、lang） |
| 宿主→插件 | `{ type: "host_unload", token: "<uuid>" }` | 组件卸载前通知 |
| 插件→宿主 | `{ type: "resize", token: "<uuid>", height: 200 }` | 请求宿主调整 iframe 高度 |
| 插件→宿主 | `{ type: "navigate", token: "<uuid>", path: "/posts" }` | 请求宿主导航到指定路由 |

安全措施：
- 所有消息必须携带初始化时下发的 token，宿主校验 token 匹配
- 宿主校验 `event.origin === window.location.origin`
- 卸载流程：宿主发送 host_unload → 等待不超过 1 秒 → 移除 iframe DOM

## 附录 C: 6 个注入点对应的 UI 位置

| slot ID | 页面/组件 | 插入位置 |
|---------|-----------|----------|
| `dashboard.widget` | Posts.tsx | 统计卡片与 PageHeader 之间 |
| `post_editor.sidebar` | PostEditor.tsx | 右侧面板（分类标签下方） |
| `sidebar.menu_item` | Sidebar.tsx | 导航区底部（"插件"分组标题下） |
| `settings.sub_section` | Settings.tsx | 界面设置与数据备份之间 |
| `post_list.action_bar` | Posts.tsx | PageHeader 的操作按钮区域（新建按钮左侧） |
| `login.form_below` | Login.tsx | 登录表单与底部"返回首页"链接之间 |
