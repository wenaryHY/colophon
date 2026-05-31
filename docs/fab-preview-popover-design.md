# FAB 浮窗预览 — 完整交互方案设计

> 版本：v1.0 | 日期：2026-06-01

---

## 1. 背景与现有架构

### 1.1 当前 FAB 架构

```
FabContainer (position: fixed, left/top 由 useDraggable 驱动)
├── Overlay        (position: fixed, inset:0, zIndex:999)    — 仅菜单展开时显示
├── SpeedDial      (position: absolute, 相对容器居中)         — 展开的子菜单项
└── MainButton     (56px 圆形, zIndex:1001)                   — 拖拽手柄 + 触发器
```

### 1.2 当前预览架构

```
PreviewProvider (App 根级)
├── PreviewContext (content, contentType, theme, themeConfig, device, zoom...)
│   └── registerScene(id, { getContent, getContentType, getTheme, getThemeConfig })
│       → rAF 轮询同步内容到 context
│
PreviewRenderer (mode: 'inline' | 'modal' | 'new-tab')
└── iframe srcdoc="由前端拼接的完整 HTML"  ← 纯前端渲染，不经后端 API
```

### 1.3 FAB 现有使用位置

- `PostEditor.tsx` — 单个 action: `preview`，点击切换 `showPreview`
- `ThemeDetail.tsx` — 单个 action: `preview`，点击切换 `showPreview`

> 注：当前 `showPreview` 状态由各自页面管理，点击 FAB 只是切换布尔值，**没有浮窗预览**。

---

## 2. 闪现 bug 根因分析

### 2.1 问题复现路径

```
1. 用户拖拽 FAB 到屏幕某位置
2. 松手 → FAB 闪现到左上角 (0, 0)
```

### 2.2 根因

**根因链路（双重 bug 组合）**：

#### 2.2.1 第一层：闭包陈旧导致 `isDragging` 卡在 `true`

`useDraggable.ts` 中，`isDragging` 是 React **state**，在 `onPointerMove` 和 `onPointerUp` 中作为守卫条件：

```typescript
// onPointerMove (line 262)
const onPointerMove = useCallback((e: React.PointerEvent) => {
  if (!isDragging) return;  // ← closure 中的旧值
  ...
}, [isDragging, elementWidth, elementHeight]);

// onPointerUp (line 307)
const onPointerUp = useCallback((e: React.PointerEvent) => {
  if (!isDragging) return;  // ← closure 中的旧值
  ...
}, [isDragging, id, ...]);
```

**时序问题**：

```
事件帧 1: onPointerDown 执行
  → setIsDragging(true)        // React 调度重渲染（尚未执行）
  → setPointerCapture(id)      // 浏览器捕获后续指针事件

事件帧 2: onPointerMove 执行   // ← 闭包中 isDragging 仍是 false！
  → if (!isDragging) return;   // 提前退出，丢弃此次移动

事件帧 3: React 重渲染完成
  → 新闭包 isDragging = true

事件帧 4: onPointerUp 执行     // 如果用户碰巧此时松手
  → if (!isDragging) return;   // ← 可能是旧/新闭包，取决于时序
```

最坏情况（快速点击、没拖拽）：

```
onPointerDown → setIsDragging(true)
onPointerUp   → 闭包是旧的，isDragging = false → return  // 未执行清理逻辑！
→ React 重渲染后 isDragging 永久 = true
→ setIsDragging(false) 永远不会被调用
```

#### 2.2.2 第二层：`latestX`/`latestY` 缺失时默认为 `(0, 0)`

```typescript
// dragStateRef 初始化 (line 209)
const dragStateRef = useRef({
  latestX: 0,   // ← 默认 0！
  latestY: 0,
  ...
});
```

在 `handlePointerUp` 中：

```typescript
// line 318-319
let finalX = state.latestX;  // 如果从未 move，就是 0
let finalY = state.latestY;  // 如果从未 move，就是 0

setPosition({ x: finalX, y: finalY });   // → 设置到 (0, 0)
savePosition(id, { x: finalX, y: finalY }); // → 持久化到 localStorage
```

#### 2.2.3 致命组合

```
第 1 次点击（无拖拽）:
  isDragging 卡在 true（2.2.1）
  
第 2 次点击:
  onPointerDown → 设置 ref state.initialX/Y = 正确位置，setIsDragging(true)
  ↓ 用户手指/鼠标未移动（或 move 被丢弃）
  ↓ latestX/Y 仍为 0
  onPointerUp → isDragging = true（第2次闭包已刷新）
    → finalX = 0 ← 使用了 latestX 默认值！
    → setPosition({ x: 0, y: 0 }) → FAB 闪现左上角
    → savePosition → (0, 0) 持久化到 localStorage
```

### 2.3 修复方案（最小改动）

修改 `useDraggable.ts`，三处改动：

**改动 1**：在 `onPointerDown` 中初始化 `latestX`/`latestY` 为当前位置

```typescript
// onPointerDown 中新增 (约 line 235 之后)
state.latestX = position.x;  // 初始化为当前位置，防止未移动就松手
state.latestY = position.y;
```

**改动 2**：用 ref 替代 state 作为 `isDragging` 的守卫条件

```typescript
// 新增 ref
const isDraggingRef = useRef(false);

// onPointerDown 中
isDraggingRef.current = true;
setIsDragging(true);

// onPointerMove / onPointerUp 中
if (!isDraggingRef.current) return;  // ← 直接用 ref，无闭包陈旧问题

// onPointerUp 末尾
isDraggingRef.current = false;
setIsDragging(false);
```

**改动 3**：`onPointerUp` 中增加兜底——如果 `latestX/Y` 从未被 move 更新，回退到 `initialX/Y`

```typescript
// onPointerUp 中，line 318 改为：
let finalX = hasMovedRef.current
  ? state.latestX
  : state.initialX;  // ← 未移动时回退到初始位置
let finalY = hasMovedRef.current
  ? state.latestY
  : state.initialY;
```

> 这三处改动是**纯粹 bug 修复**，不影响现有拖拽功能，可以独立 PR。

---

## 3. 浮窗定位策略对比

| 方案 | 实现方式 | 优点 | 缺点 | 推荐 |
|------|---------|------|------|------|
| **A: Portal + 绝对坐标** | 通过 `createPortal` 渲染到 `document.body`，JS 计算相对于 FAB 的屏幕坐标，`position: fixed` 定位 | 无 z-index 嵌套问题；浮窗不被父元素裁剪；可以独立管理生命周期 | 需要监听 FAB 位置变化 + resize；移动端全屏需要额外逻辑 | ⭐ **推荐** |
| **B: FAB 子元素** | 浮窗是 `FabContainer` 的子元素，`position: absolute` 相对于容器（`position: fixed`）定位 | 自然跟随拖拽；不需要坐标计算；代码最简 | 移动端全屏需要特殊处理（无法突破父元素）；浮窗尺寸受容器约束 | 不推荐 |
| **C: Popper 类库** | 引入 `@floating-ui/react`，使用其 `useFloating` + `shift`/`flip` middleware | 碰撞检测极其完善；箭头指向；动画友好 | 额外依赖 ~12KB；本项目场景简单，过度设计 | 不推荐 |

### 推荐：方案 A（Portal + 动态坐标）

**理由**：
1. 桌面端浮窗依附 FAB → JS 计算 `position: fixed` 坐标，监听 FAB 位置 + resize
2. 平板端居中浮窗 → 直接 `position: fixed` 居中，忽略 FAB 坐标
3. 移动端全屏 → `position: fixed; inset:0`，无需计算
4. 三种模式共用同一组件，只需切换定位模式
5. 未来如果需要 FAB 和浮窗分别在不同 React 树中（如两个不同的 Portal），架构也支持

---

## 4. 定位算法

### 4.1 基础算法

```
输入：
  fabX, fabY        — FAB 左上角屏幕坐标
  fabSize = 56      — FAB 尺寸
  prevW, prevH      — 浮窗宽高
  gap = 16          — 浮窗与 FAB 的间距
  vw, vh            — 视口宽高

输出：
  popoverX, popoverY — 浮窗 fixed 定位的 left, top
```

**优先级策略**（按可用空间排序）：

```
1. 下方可用?  (fabY + fabSize + gap) + prevH <= vh
   → popoverY = fabY + fabSize + gap

2. 上方可用?  fabY - gap - prevH >= 0
   → popoverY = fabY - gap - prevH
   
3. 右方可用?  (fabX + fabSize + gap) + prevW <= vw
   → popoverX = fabX + fabSize + gap
   → popoverY = clamp(fabY, 0, vh - prevH)

4. 左方可用?  fabX - gap - prevW >= 0
   → popoverX = fabX - gap - prevW
   → popoverY = clamp(fabY, 0, vh - prevH)

5. 都不够 → 紧急模式：对齐底部 + 缩小高度
   → popoverY = max(0, vh - prevH - 8)
   → popoverX = clamp(fabX + fabSize/2 - prevW/2, 0, vw - prevW)
```

**水平对齐**：
```
如果位置是"上方"或"下方"：
  浮窗优先与 FAB 右对齐:
    popoverX = fabX + fabSize - prevW
  如果超出视口 → 改为左对齐:
    popoverX = fabX
  如果仍超出 → clamp 到视口内
```

### 4.2 完整算法伪代码

```typescript
interface PositionResult {
  x: number;
  y: number;
  direction: 'up' | 'down' | 'left' | 'right';
}

function calculatePopoverPosition(
  fabRect: { x: number; y: number; size: number },
  popoverSize: { width: number; height: number },
  viewport: { width: number; height: number },
  gap = 16,
): PositionResult {
  const { x: fx, y: fy, size: fs } = fabRect;
  const { width: pw, height: ph } = popoverSize;
  const { width: vw, height: vh } = viewport;

  // 1. 优先下方
  if (fy + fs + gap + ph <= vh) {
    return {
      x: clamp(fx + fs - pw, 0, vw - pw),
      y: fy + fs + gap,
      direction: 'down',
    };
  }

  // 2. 其次上方
  if (fy - gap - ph >= 0) {
    return {
      x: clamp(fx + fs - pw, 0, vw - pw),
      y: fy - gap - ph,
      direction: 'up',
    };
  }

  // 3. 右侧
  if (fx + fs + gap + pw <= vw) {
    return {
      x: fx + fs + gap,
      y: clamp(fy + fs / 2 - ph / 2, 0, vh - ph),
      direction: 'right',
    };
  }

  // 4. 左侧
  if (fx - gap - pw >= 0) {
    return {
      x: fx - gap - pw,
      y: clamp(fy + fs / 2 - ph / 2, 0, vh - ph),
      direction: 'left',
    };
  }

  // 5. 兜底：粘底
  return {
    x: clamp(fx + fs / 2 - pw / 2, 0, vw - pw),
    y: Math.max(0, vh - Math.min(ph, vh * 0.6) - 8),
    direction: 'down',
  };
}

function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(value, max));
}
```

---

## 5. 响应式行为设计

| 断点 | 模式 | 行为 |
|------|------|------|
| **桌面** (>1024px) | 依附 FAB | 浮窗与 FAB 相邻，可拖动右下角调整尺寸（min 320×240, max 800×80%vh），有最小/最大限制记忆 |
| **平板** (768-1024px) | 居中浮窗 | `position: fixed; left: 50%; transform: translateX(-50%); width: 80vw; max-width: 720px`，高度适应内容，max-height 70vh |
| **移动** (<768px) | 全屏面板 | `position: fixed; inset: 0; border-radius: 0`，顶部有关闭按钮和"新标签页打开"按钮 |

### 5.1 响应式切换

```typescript
function useResponsiveMode(): 'desktop' | 'tablet' | 'mobile' {
  const [mode, setMode] = useState(getMode());

  useEffect(() => {
    const onResize = () => setMode(getMode());
    window.addEventListener('resize', onResize);
    return () => window.removeEventListener('resize', onResize);
  }, []);

  return mode;
}

function getMode(): 'desktop' | 'tablet' | 'mobile' {
  const w = window.innerWidth;
  if (w < 768) return 'mobile';
  if (w < 1024) return 'tablet';
  return 'desktop';
}
```

---

## 6. 推荐架构方案

### 6.1 组件树

```
App (PreviewProvider)
├── AdminLayout
│   ├── Sidebar
│   ├── <Outlet>  ← 各页面（PostEditor, ThemeDetail...）
│   └── FabContainer (position: fixed, zIndex: 1000)
│       ├── Overlay (菜单遮罩)
│       ├── SpeedDial (子菜单项)
│       └── MainButton (拖拽手柄)
│
└── <FabPreviewPortal>   ← Portal 到 document.body, zIndex: 1002
    ├── 桌面模式：position: fixed, 依附 FAB 位置
    ├── 平板模式：position: fixed, 居中 80% 宽
    └── 移动模式：position: fixed, 全屏
```

### 6.2 数据流

```
                    ┌──────────────────────────┐
                    │   FabPreviewContext       │  ← 新增 Context
                    │  isOpen: boolean          │
                    │  open() / close()          │
                    │  toggle()                  │
                    └──────────┬───────────────┘
                               │
            ┌──────────────────┼──────────────────┐
            │                  │                   │
     FabContainer         PostEditor         ThemeDetail
     (点击 preview         调用 open()         调用 open()
      action 时              切换浮窗可见        切换浮窗可见
      调用 toggle/open)        性                 性
            │                  │                   │
            └──────────────────┴───────────────────┘
                               │
                    ┌──────────▼───────────┐
                    │  FabPreviewPortal    │
                    │  (FabPreviewPopover) │
                    │                      │
                    │  读取 PreviewContext │
                    │  (content, theme...) │
                    │                      │
                    │  读取 FabPosition    │
                    │  (FAB 的屏幕坐标)    │
                    └──────────┬───────────┘
                               │
                    ┌──────────▼───────────┐
                    │  iframe              │
                    │  src="/api/v1/       │
                    │      preview?..."      │
                    │  或 srcdoc (兜底)    │
                    └──────────────────────┘
```

> **关键设计决策**：`FabPreviewContext` 用 Context 而非各页面本地 state，因为多个页面（PostEditor、ThemeDetail、未来更多）的 FAB preview action 都应该打开同一个浮窗机制。

### 6.3 与现有 PreviewContext 的关系

`FabPreviewContext` **只管理浮窗的 打开/关闭 状态和 FAB 位置**，不重复管理预览内容。

预览内容仍然由现有的 `PreviewContext` 管理：
- PostEditor 通过 `preview.registerScene('post-editor', ...)` 注册场景
- ThemeDetail 通过 `preview.registerScene('theme-detail', ...)` 注册场景
- `FabPreviewPopover` 从 `PreviewContext` 读取 `content, contentType, theme, ...` 并渲染

---

## 7. "新标签页打开" 流程

### 7.1 流程设计

```
用户点击浮窗内 [在新标签页打开] 按钮
  │
  ├─► 1. 从 PreviewContext 收集当前预览数据
  │     { content, contentType, theme, themeConfig, device, zoom }
  │
  ├─► 2. 写入 sessionStorage
  │     sessionStorage.setItem('inkforge-preview-data', JSON.stringify(data))
  │
  ├─► 3. 打开新标签页
  │     window.open('/preview', '_blank')
  │
  ├─► 4. 关闭 FAB 浮窗
  │     FabPreviewContext.close()
  │
  └─► 5. 新标签页 /preview 加载
        ├── 读取 sessionStorage 中的预览数据
        ├── 如果有 → 调用后端 /api/v1/preview API 渲染完整页面
        └── 如果无 → 显示空状态
```

### 7.2 新标签页 /preview 路由

需要新增一个前端路由（已有 `/preview` 的 `window.open` 调用，但未实现对应页面）：

- 路由：`/preview`（在 AdminGate 之外，不需要登录）
- 组件：`StandalonePreview`（从 sessionStorage 读取数据，请求后端 API 渲染）

---

## 8. 需要修改 / 新增的文件清单

### 8.1 Bug 修复（独立 PR）

| 文件 | 改动 |
|------|------|
| `src/admin/ui/src/fab/useDraggable.ts` | 修复 `isDragging` 闭包陈旧 + `latestX/Y` 初始值问题（3 处改动，见 2.3） |

### 8.2 新增文件

| 文件 | 说明 |
|------|------|
| `src/admin/ui/src/fab/FabPreviewContext.tsx` | `FabPreviewContext` + `FabPreviewProvider` + `useFabPreview` hook |
| `src/admin/ui/src/fab/FabPreviewPopover.tsx` | 浮窗组件：定位计算、响应式、iframe 渲染、新标签页按钮 |
| `src/admin/ui/src/fab/usePopoverPosition.ts` | 定位算法 hook（4.2 节） |

### 8.3 修改现有文件

| 文件 | 改动 |
|------|------|
| `src/admin/ui/src/fab/FabContainer.tsx` | 接入 `FabPreviewContext`，点击 preview action 时调用 `open()`；暴露 FAB ref 供位置计算 |
| `src/admin/ui/src/fab/index.ts` | 导出新的 `FabPreviewProvider`、`FabPreviewPopover`、`useFabPreview` |
| `src/admin/ui/src/App.tsx` | 在 `PreviewProvider` 内包裹 `FabPreviewProvider`；在 `<Routes>` 同级渲染 `<FabPreviewPopover />`；新增 `/preview` 路由 |
| `src/admin/ui/src/pages/PostEditor.tsx` | 移除 `showPreview` 本地 state，改用 `useFabPreview()` |
| `src/admin/ui/src/pages/ThemeDetail.tsx` | 同上 |

### 8.4 可选新增（后端 API，若不新增则用现有 srcdoc 兜底）

| 文件 | 说明 |
|------|------|
| `src/modules/preview/...` (Rust) | 后端预览 API：`GET /api/v1/preview?content_type=markdown&content=...&theme=...` 返回渲染后的 HTML |
| `src/admin/ui/src/pages/StandalonePreview.tsx` | 新标签页的独立预览页面 |

---

## 9. 浮窗组件详细设计

### 9.1 FabPreviewPopover 组件接口

```typescript
interface FabPreviewPopoverProps {
  /** FAB 元素的 ref，用于获取屏幕坐标 */
  fabRef: React.RefObject<HTMLElement | null>;
}
```

### 9.2 浮窗内部布局

```
┌─────────────────────────────────────────────┐
│ 🔍 预览          [↗ 新标签页打开]  [✕ 关闭] │ ← 标题栏
├─────────────────────────────────────────────┤
│                                             │
│   ┌───────────────────────────────────┐     │
│   │                                   │     │
│   │       iframe 预览区域             │     │
│   │       (可滚动)                    │     │
│   │                                   │     │
│   └───────────────────────────────────┘     │
│                                             │
├─────────────────────────────────────────────┤
│  ◇ 主题: default  设备: 桌面  ▼  缩放: 100%│ ← 底部工具栏（复用现有 PreviewContext 控制）
└─────────────────────────────────────────────┘
                              └─ 桌面端右下角可拖动调整尺寸
```

### 9.3 iframe 策略

```typescript
// 优先尝试后端渲染，失败时 fallback 到 srcdoc 渲染
const iframeSrc = useMemo(() => {
  // 后端 API 方式（需要后端实现）
  // return `/api/v1/preview?content=${encodeURIComponent(content)}&content_type=${contentType}&theme=${theme}`;

  // 当前阶段：使用 srcdoc（与现有 PreviewRenderer 一致）
  return undefined; // 用 srcdoc 属性替代 src
}, [content, contentType, theme]);

const srcDoc = useMemo(
  () => buildPreviewHtml(content, contentType, theme, themeConfig),
  [content, contentType, theme, themeConfig]
);

// 通过 postMessage 增量更新（避免整个 iframe 重载）
useEffect(() => {
  if (iframeRef.current?.contentWindow && iframeLoaded) {
    const html = contentType === 'markdown'
      ? simpleMarkdownToHtml(content)
      : content;
    iframeRef.current.contentWindow.postMessage({
      type: 'CONTENT_UPDATE',
      payload: { html, content, contentType },
    }, '*');
  }
}, [content, contentType, iframeLoaded]);
```

---

## 10. 代码示例

### 10.1 FabPreviewContext

```typescript
// src/admin/ui/src/fab/FabPreviewContext.tsx
import { createContext, useCallback, useContext, useRef, useState, type ReactNode } from 'react';

interface FabPreviewState {
  isOpen: boolean;
  open: () => void;
  close: () => void;
  toggle: () => void;
  fabRef: React.RefObject<HTMLDivElement | null>;
}

const FabPreviewContext = createContext<FabPreviewState | null>(null);

export function FabPreviewProvider({ children }: { children: ReactNode }) {
  const [isOpen, setIsOpen] = useState(false);
  const fabRef = useRef<HTMLDivElement>(null);

  const open = useCallback(() => setIsOpen(true), []);
  const close = useCallback(() => setIsOpen(false), []);
  const toggle = useCallback(() => setIsOpen(prev => !prev), []);

  return (
    <FabPreviewContext.Provider value={{ isOpen, open, close, toggle, fabRef }}>
      {children}
    </FabPreviewContext.Provider>
  );
}

export function useFabPreview() {
  const ctx = useContext(FabPreviewContext);
  if (!ctx) throw new Error('useFabPreview must be used within FabPreviewProvider');
  return ctx;
}
```

### 10.2 浮窗定位 hook 核心实现

```typescript
// src/admin/ui/src/fab/usePopoverPosition.ts
import { useEffect, useMemo, useState } from 'react';
import type { CSSProperties } from 'react';

const FAB_SIZE = 56;
const GAP = 16;
const DEFAULT_SIZE = { width: 420, height: 480 };
const MOBILE_BP = 768;
const TABLET_BP = 1024;

interface PositionResult {
  style: CSSProperties;
  direction: 'up' | 'down' | 'left' | 'right';
}

export function usePopoverPosition(
  fabElement: HTMLElement | null,
  isOpen: boolean
): PositionResult {
  const [viewport, setViewport] = useState({ w: window.innerWidth, h: window.innerHeight });

  useEffect(() => {
    const onResize = () => setViewport({ w: window.innerWidth, h: window.innerHeight });
    window.addEventListener('resize', onResize);
    return () => window.removeEventListener('resize', onResize);
  }, []);

  // 桌面端：依附 FAB
  const desktopStyle = useMemo(() => {
    if (!fabElement || !isOpen) return { display: 'none' };

    const rect = fabElement.getBoundingClientRect();
    const { w: vw, h: vh } = viewport;
    const pw = DEFAULT_SIZE.width;
    const ph = DEFAULT_SIZE.height;

    let x: number, y: number;

    // 下方
    if (rect.bottom + GAP + ph <= vh) {
      y = rect.bottom + GAP;
    }
    // 上方
    else if (rect.top - GAP - ph >= 0) {
      y = rect.top - GAP - ph;
    }
    // 兜底
    else {
      y = Math.max(0, vh - ph - 8);
    }

    // 水平：优先右对齐 FAB
    x = rect.right - pw;
    if (x < 8) x = rect.left;
    if (x + pw > vw - 8) x = vw - pw - 8;
    if (x < 8) x = 8;

    return {
      position: 'fixed' as const,
      left: x,
      top: y,
      width: pw,
      height: ph,
      zIndex: 1002,
    };
  }, [fabElement, isOpen, viewport]);

  // 平板：居中
  const tabletStyle: CSSProperties = {
    position: 'fixed',
    left: '50%',
    top: '10vh',
    transform: 'translateX(-50%)',
    width: '80vw',
    maxWidth: 720,
    maxHeight: '70vh',
    zIndex: 1002,
  };

  // 移动端：全屏
  const mobileStyle: CSSProperties = {
    position: 'fixed',
    inset: 0,
    zIndex: 1002,
    borderRadius: 0,
  };

  const style = viewport.w < MOBILE_BP
    ? mobileStyle
    : viewport.w < TABLET_BP
      ? tabletStyle
      : desktopStyle;

  return { style: isOpen ? style : { display: 'none' }, direction: 'up' };
}
```

### 10.3 FabContainer 改动要点

```typescript
// FabContainer.tsx — 接入 FabPreviewContext
import { useFabPreview } from './FabPreviewContext';

export function FabContainer({ actions, ... }: FabContainerProps) {
  const { open: openPreview, fabRef } = useFabPreview();

  // 修改 preview action 的 onClick
  const handleActionClick = useCallback((action: FabAction) => {
    setIsOpen(false);        // 关闭 SpeedDial
    if (action.id === 'preview') {
      openPreview();         // 打开浮窗预览
    } else {
      action.onClick();
    }
  }, [openPreview]);

  // ref 合并：用于拖拽的 dragRef + 用于浮窗定位的 fabRef
  // 需要在容器 div 上同时绑定

  return (
    <div
      ref={(node) => {
        // 合并 ref：dragRef + fabRef
        dragRef.current = node;
        fabRef.current = node;
      }}
      style={containerStyle}
    >
      ...
    </div>
  );
}
```

### 10.4 App.tsx 改动要点

```typescript
// App.tsx — 添加 FabPreviewProvider 和路由
import { FabPreviewProvider, FabPreviewPopover } from './fab';
import StandalonePreview from './pages/StandalonePreview'; // 可选

export default function App() {
  return (
    <BrowserRouter basename="/admin">
      <SlotsProvider>
        <PreviewProvider>
          <FabPreviewProvider>
            <Routes>
              {/* 现有路由... */}
              <Route path="/preview" element={<StandalonePreview />} />
            </Routes>
            {/* 浮窗渲染在 Routes 同级，不受路由切换影响 */}
            <FabPreviewPopover />
          </FabPreviewProvider>
        </PreviewProvider>
      </SlotsProvider>
    </BrowserRouter>
  );
}
```

---

## 11. 方案优缺点总结

| 方面 | 决策 | 理由 |
|------|------|------|
| 浮窗定位 | Portal + `position: fixed` + JS 坐标计算 | 三种响应式模式统一架构；不依赖第三方库 |
| FAB 拖拽跟随 | 通过 ref 获取 FAB 的 `getBoundingClientRect()`，在浮窗中每帧更新 | FAB 和浮窗分离渲染，不互相阻碍渲染；`useDraggable` 的 `requestAnimationFrame` 已保证拖拽流畅 |
| 预览内容传递 | 继续使用现有 `PreviewContext` + `registerScene` 模式 | 页面与浮窗解耦，浮窗不关心数据来源 |
| 新标签页流程 | sessionStorage → window.open('/preview') → 新页面读取 | 已有此模式（`PreviewContext.openInNewTab`），无需重新设计 |
| iframe 渲染方式 | 阶段一用 srcdoc + postMessage 增量更新（与现有一致）；阶段二接入后端 `/api/v1/preview` API | 不阻塞前端功能交付；后端 API 可后续补充 |
| z-index 分层 | Overlay=999, FAB=1000, MainButton=1001, 浮窗=1002 | 浮窗盖在 FAB 之上，避免被遮罩遮盖 |
| 拖拽时浮窗行为 | 拖拽过程中浮窗保持跟随（通过 rAF 实时读取 FAB rect） | 用户体验需要；若性能有问题可改为拖拽时隐藏、松手后显示 |
