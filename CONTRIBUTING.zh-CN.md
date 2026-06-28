# 贡献给 Colophon

**[English](CONTRIBUTING.md)** | **[中文](CONTRIBUTING.zh-CN.md)**

感谢你对 Colophon 的关注！我们欢迎各种形式的贡献。

---

## 新手友好任务 (Good First Issues)

在 [Issue 追踪器](https://github.com/wenaryHY/colophon/issues) 中寻找带有 `good first issue` 标签的任务。

适合新手的切入点：
- 修复拼写错误或改进错误提示信息
- 添加缺失的国际化翻译（i18n）
- 为未覆盖测试的函数编写测试
- 改进文档注释

不确定某项修改是否合适？请先开一个 Issue 进行讨论。

---

## 开始之前

- Rust 1.75+
- Node.js 22+
- SQLite 3

```bash
git clone https://github.com/wenaryHY/colophon.git
cd colophon
cd src/admin/ui && npm ci && cd -
```

---

## 开发工作流

同时启动后端服务与前端 Vite 开发服务器：

```bash
npm install
npm run dev
```

**生产环境构建：**

```bash
cargo build --release -p colophon
```

管理面板的前端静态资源会在 `cargo build` 时自动嵌入二进制文件中。

**运行测试：**

```bash
# 后端测试
cargo test -p colophon

# 前端类型检查与构建
cd src/admin/ui && npm run build
```

**Lint 检查：**

```bash
# Rust
cargo clippy -- -D warnings

# 前端
cd src/admin/ui && npm run lint
```

---

## 代码编写原则

### 函数长度

每个函数（不含注释和空行）**不能超过 40 行**。短函数更易于测试、调试和理解。如果函数过长，请将其拆分为多个子函数。

### 命名规范

优先级：**正确性 > 准确性 > 一致性**。名称必须能准确地描述其用途、返回值和副作用。宁可使用长命名，也不要为了简短而牺牲正确性。
- 推荐示例：`validateThemeSlugIsInstalledAndSafeForPreviewRendering`
- 避开命名：`data`、`item`、`result`

### 禁止魔术数字

所有的数字常量都必须使用具名常量声明。不允许使用 `if attempts > 5`；应使用 `const MAX_LOGIN_ATTEMPTS: i32 = 5;` 并进行引用。

### 国际化 (i18n)

所有面向用户的文本必须通过 i18n系统同时支持中文和英文。严禁在代码中硬编码任何对用户可见的文本字符串。

### 后端输入验证

后端必须对所有传入数据进行独立的验证、清理和授权。前端的验证仅用于提升用户体验，而不是安全边界。每个 API 端点都必须检查身份验证、输入约束以及资源的所有权。

---

## Rust 开发指南

- **零成本抽象**：使用 traits + 泛型来实现可替换组件（如数据库、存储或协议）。优先采用编译期多态；除非必要，否则应避免使用 `Box<dyn Trait>`。
- **禁止使用 `unsafe`**：Colophon 运行于不受信任的客户端环境。如果您认为确有必要使用 `unsafe`，请在您的 PR 中详细解释原因。
- **Clippy 检查**：在提交之前，`cargo clippy -- -D warnings` 必须通过且无任何警告。

---

## 前端开发指南

- **禁用 `any`**：使用 `unknown` 并通过类型收窄来处理类型。
- **响应式设计**：管理面板必须兼容 320px 视口宽度。请使用相对单位，并在 320px / 768px / 1280px 断点下进行测试。在触摸设备上，关键性的交互请勿依赖悬停（hover）状态。

---

## 测试要求

- 新功能必须附带测试，覆盖主路径以及至少一个边界情况。
- 修复 Bug 时必须附带回归测试，以重现该 Bug（红-绿-重构流程）。
- 后端测试：`cargo test -p colophon`。前端测试：`cd src/admin/ui && npm test`。

---

## Commit 信息规范

接受英文或中文提交。保持中性风格 —— 请勿包含任何 AI 工具名称。主旨行控制在 72 字符以内。

```
<type>: <简短描述>
```

示例：`feat: add slug collision detection`，`fix: 修复 IME 输入时光标位置问题`。

---

## 分支命名规范

格式：`type/description`（全小写，使用 kebab-case 分隔）。

| 类型 | 用途 |
|------|---------|
| `feat` | 新功能 |
| `fix` | Bug 修复 |
| `refactor` | 代码重构 |
| `docs` | 仅文档更新 |
| `chore` | 构建、CI、工具等 |
| `test` | 新增或更新测试 |

示例：`feat/slug-editor`，`fix/ime-cursor`，`refactor/auth-middleware`。

---

## AI 辅助策略

您可以辅助使用 AI 工具，但您必须完全理解所提交的每一行代码。

**如果使用了 AI 辅助：**
1. 请在 PR 描述中公开披露哪些部分使用了 AI、以及如何使用的。
2. 准备好在被问及时解释代码的任何部分。
3. 提交前清除所有 AI 生成的注释、虚假引用或工具特定的标记。
4. 仔细审查 —— AI 生成的代码在错误处理和边界情况下经常包含细微的安全漏洞或 Bug。

---

## Pull Request 检查清单

**代码质量**
- [ ] 所有函数 ≤ 40 行
- [ ] 无魔术数字，常量皆有命名
- [ ] 后端独立验证所有输入数据
- [ ] 所有面向用户的文本皆已国际化（支持中英文）

**技术标准**
- [ ] Rust：不含 `unsafe`（除非有合理解释）；`cargo clippy -- -D warnings` 通过
- [ ] TypeScript：不含 `any` 类型
- [ ] UI 在 320px 宽度下工作正常（如适用）

**测试**
- [ ] 已添加相关测试；`cargo test -p colophon` 通过
- [ ] `cd src/admin/ui && npm run build` 构建成功
- [ ] 不含提交的密钥、Token 或任何 AI 工具留下的痕迹

---

## 哪些情况会被退回修改

| 问题 | 解决方法 |
|-------|------------|
| **缺少 i18n** | 使用 `t()` 函数和 i18n 系统进行重构 |
| **后端信任前端输入** | 在后端增加独立的输入验证逻辑 |
| **函数过长** | 将长函数拆分为多个较小的函数 |
| **命名不清晰** | 使用能够准确描述用途的完整英文单词进行命名 |
| **引入不必要的依赖** | 优先使用标准库或项目内已有的依赖库 |
| **移动端布局损坏** | 采用响应式布局，并在 320px 视口下调试通过 |

这些要求并非绝对拒绝 —— 我们会引导您一步步完成修改。

---

## 需要帮助？

- 在 [Issues](https://github.com/wenaryHY/colophon/issues) 中提问
- 在 [Discussions](https://github.com/wenaryHY/colophon/discussions) 中讨论想法
- 参考项目中已有的类似实现代码

---

## 许可证

一旦贡献，即代表您同意您的贡献将基于 [GNU Affero General Public License v3.0](LICENSE) (AGPLv3) 进行开源授权。

---

**每一次贡献都会让 Colophon 变得更好。谢谢！**
