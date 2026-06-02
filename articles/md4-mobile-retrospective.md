# 毛玻璃、Popover 与 cargo run 的歧路——MD4 移动端重构血泪记

上篇结尾我说"下个月见"。(´・ω・`)  
flag 这种东西，立得越快倒得越快——第二天就开始做移动端适配了。以下是全过程复盘，含五个小时的 debug、2100 行原型、以及一系列"我以为能跑"的惨案。

## isMobile 的幽灵：一场五小时的 debug 马拉松

最初的想法极其朴素：给 PostEditor 加几个 `isMobile ? ... : ...` 三元判断，改改 grid 布局，完事。(◔_◔)

然后我度过了此生最漫长的五个小时——

1. `isMobile` 明明 `console.log` 是 `true`，DOM 纹丝不动
2. 换了两个独立容器 → 不动
3. 硬编码 `isMobile = true` → 不动
4. 清浏览器缓存、SW 缓存 → 不动
5. 查 `cargo watch` 工作目录 → 对
6. 查 `npm build` 输出目录 → 也对

(╯°□°)╯︵ ┻━┻

第五个小时，偶然发现 `cargo run` 的工作目录跑在 `src/admin/ui/` 下面，不是项目根——静态资源全加载了旧版本。修好之后——**还是不动**。

最终定位到真凶：

```jsx
// 这段代码看起来人畜无害
// 但它不会触发 React 的 DOM 更新
style={{ gridTemplateColumns: isMobile ? undefined : '1fr 260px' }}
```

同一个组件内通过三元切换 inline style 属性值，React 根本感知不到布局变化——它 diff 的是 style 对象引用，而不是 CSS 属性的语义等价。那个 `isMobile` 判断从一开始就没用对。(；∀；) 五个小时买了一个常识。

## 原型先行：2100 行的"我要的就是这个效果"

既然 inline style 这条路走不通，决定推翻重来——先做纯 HTML 原型，设计定稿后再移植 React。

原型选了 **Floating UI**（Popper.js 的后代，轻量且 tree-shaking 友好）做 popover 定位，手写了 2100 行：

- 所有 emoji → Sharp 风格 SVG 图标
- AppBar **毛玻璃效果**：`backdrop-filter: blur(16px)` + 半透明背景
- **底部工具栏**：B/I/U 基础格式，展开行 → H 标题 / 引用 / 列表 / 链接 / 图片 / 代码 / 预览
- 预览 **Bottom Sheet** 从底部滑出
- AppBar 菜单 + **二级滑动子菜单**（分类 / 标签 / 发布状态）
- 摘要栏可折叠

最大的坑是 popover 的 `overflow: hidden` 裁剪——移动端 WebKit 上 floating 元素会被父容器裁掉，桌面端 Chrome 却没事。调了半天 `z-index` 和 `overflow` 的组合拳。(；一_一) 最后发现：**父容器必须移除 `overflow` 限制，`z-index` 救不了被裁切的子元素**——这是 CSS 层叠上下文的硬规则，不是 bug。

## 移植：原型很美好，React 很骨感

从 prototype 到 React 的移植才是地狱难度。每一个"理所当然"都变成了 bug：

- **`overflow: 'hidden'` 又双叒裁剪 popover**：修了两次——第一次以为加 `z-index` 能解决，第二次才溯源到 5 层之外的祖容器
- **源码模式禁用链**：B/I/U 整行 `pointerEvents` 禁用导致同行的展开按钮也点不了
- **FAB 与底部工具栏互殴**：移动端 FAB 和底部工具栏都在抢 `position: fixed` 的底部位置
- **AppBar 返回按钮 vs sidebar hamburger**：两个按钮叠在同一坐标
- **`showTabBar={false}` 的连锁反应**：隐藏了 MarkdownEditor 的模式切换按钮，但外部没有任何切换入口——被迫加了 `forcedMode` prop

(ノಠ益ಠ)ノ 每一个 bug 修完都会精准地 crash 出下一个，仿佛 bug 之间有某种共生关系。

## 编译优化与部署事故

顺手做了编译优化——加 **mold 链接器**，链接阶段从 30 秒压到 3 秒。但第一次配置忘了 WSL 里根本没装 clang（mold 依赖它），又花半小时折腾环境。

加了两个配置文件：

```toml
# .cargo/config.toml
[target.x86_64-unknown-linux-gnu]
rustflags = ["-C", "link-arg=-fuse-ld=mold"]
```

```gitattributes
*.sql text eol=lf
```

第二条是因为昨天的 **CRLF 换行符灾难**——SQLite 被 `\r\n` 搞崩的记忆还在冒烟。强制 LF 换行符，一劳永逸。

部署也没省心——`deploy.sh` 忘了同步 `themes/default/` 目录，`checkLoginStatus` 直接报错。(´ー｀)ﾌｩ 补了一版才上线。

## 教训

按疼痛级别排列：

1. **不要用 React inline style 做条件布局**。两个独立组件比条件样式可靠一万倍。inline style 的条件切换不会触发 React 的 DOM diff——它比较的是对象引用，不是 CSS 语义。
2. **原型先行**。纯 HTML 原型把设计定稿，再移植 React，比直接在 React 里试错省至少一半时间。原型阶段改一个 `overflow` 只需要刷新页面。
3. **不要相信"CSS 跨平台一致"**。`overflow: hidden` + `position: absolute` 的 popover 在移动端 WebKit 和桌面端 Chromium 表现不同。
4. **工作目录检查**。加个启动脚本里检查 `pwd` 的事，五秒钟，省五个小时。

## 结尾

虽然踩了一地坑，移动端编辑体验总算能看了——AppBar 毛玻璃反光、底部工具栏 B/I/U、预览 Sheet 从底部滑出。好不好用另说，至少和原型长得一样了。(｀・ω・´)

下次立 flag 之前，我一定先跑 `pwd`。下次说"下个月见"之前，我一定先把 `deploy.sh` 跑通。
