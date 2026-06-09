# Contributing to InkForge

**[中文](#中文版本)** | **[English](#english-version)**

---

## 中文版本

感谢你对 InkForge 的关注！我们欢迎各种形式的贡献。

**在提交 PR 之前**，请花 5 分钟阅读本文档，这会帮助你的贡献更顺利地被合并。

### 新手指引

第一次贡献开源项目？可以从这些任务开始：

- 修复文档中的错别字或不清晰的说明
- 添加单元测试
- 完善 i18n 翻译
- 修复标记为 `good first issue` 的问题

不确定某个改动是否合适？可以先开 Issue 讨论。

---

## 代码质量标准

InkForge 是一个注重代码质量的项目。我们不会为了快速合并而降低标准。

这意味着你的 PR 需要：

- 通过所有自动化检查（rustfmt、clippy、tests）
- 符合代码规范（函数长度、命名、错误处理）
- 有充分的测试覆盖

如果这是你第一次贡献，不用担心 — 我们会在 review 中提供详细反馈，帮助你达到标准。

---

## 开发环境

### 环境要求

- Rust 1.75+
- Node.js 22+
- SQLite 3

### 初始化

```bash
git clone https://github.com/wenaryHY/inkforge.git
cd inkforge
cd src/admin/ui && npm ci && cd -
```

### 启动开发服务器

```bash
# 终端 1：Rust 后端
cargo run

# 终端 2：管理面板 Vite 开发服务器
cd src/admin/ui && npm run dev
```

或者在项目根目录使用合并命令：

```bash
npm run dev
```

### 生产构建

```bash
cargo build --release -p inkforge
```

管理 UI 的生产构建会在 `cargo build` 时自动嵌入。

### 运行测试

```bash
# 后端测试
cargo test -p inkforge

# 前端类型检查 + 构建
cd src/admin/ui && npm run build
```

合并命令：

```bash
cargo test -p inkforge && cd src/admin/ui && npm run build
```

### Lint

```bash
# Rust
cargo clippy -- -D warnings

# 前端
cd src/admin/ui && npm run lint
```

---

## 代码规范

以下规范是**强制性的**，不符合的 PR 会被要求修改。

### 函数长度

- **要求**：每个函数 ≤ 40 行（不含注释和空行）
- **理由**：短函数更易测试、调试和理解
- **如何做**：超过 40 行时，考虑拆分为多个小函数

### 命名原则

优先级：**正确性 > 准确性 > 统一性**

名称必须准确描述其用途、返回值、副作用。允许超长命名，不因简短牺牲正确性。

✅ **正确示例**：
- `SESSION_STORAGE_KEY_FOR_PREVIEW_PARAMETERS_PASSED_TO_NEW_TAB` — 完整描述用途
- `validateThemeSlugIsInstalledAndSafeForPreviewRendering` — 精确描述校验逻辑
- `calculatePopoverPositionRelativeToFabButtonRect` — 纯函数不以 `use` 开头

❌ **错误示例**：
- `usePopoverPosition` — 纯计算函数不应使用 React Hook 前缀 `use`
- `data` / `item` / `result` — 无具体含义

### 禁止魔法数字

- **要求**：所有数字常量必须命名
- **错误示例**：`if attempts > 5`
- **正确示例**：
  ```rust
  const MAX_LOGIN_ATTEMPTS: i32 = 5;
  if attempts > MAX_LOGIN_ATTEMPTS { ... }
  ```

### 国际化

- **要求**：所有面向用户的文本必须通过 i18n 系统支持中英文
- **不允许**：硬编码的中文或英文字符串

```typescript
// ❌ 错误
<h1>Welcome</h1>

// ✅ 正确
<h1>{t('welcome.title')}</h1>
```

添加新的可翻译字符串时：
1. 在基础 i18n 定义中添加英文键
2. 添加中文翻译
3. 在组件中使用该键

### 后端验证所有输入

后端需要独立验证、清理和授权所有接收到的数据。前端验证只是用户体验优化，不是安全边界。

每个端点必须检查：
- 身份认证和授权
- 输入格式和约束
- 资源所有权

---

## Rust 指南

### 零成本抽象原则

当实现跨数据库、跨存储后端、跨协议等可替换组件时：

- **必须使用抽象写法**（trait + 泛型），禁止硬编码具体类型
- **抽象不应引入运行时开销** — 优先编译期多态（泛型），避免 `Box<dyn Trait>` 除非必要

✅ **正确示例**：
```rust
// 泛型参数，编译期单态化，零开销
async fn list_posts<DB: sqlx::Database>(pool: &sqlx::Pool<DB>) -> Vec<Post> { ... }
```

❌ **错误示例**：
```rust
// 硬编码具体类型 — 换数据库时需改 151 处
async fn list_posts(pool: &SqlitePool) -> Vec<Post> { ... }
```

### 避免 `unsafe`

InkForge 是服务于不可信客户端的 Web 应用，通常不需要 `unsafe` 代码。如果你认为某处确实需要，请在 PR 中详细说明理由。

### Clippy 检查

提交前运行 `cargo clippy -- -D warnings`。我们要求 PR 不引入新的 Clippy 警告。

---

## Frontend 指南

### 禁止 `any`

使用 `unknown` 并进行类型收窄。

```typescript
// ❌ 错误
function handle(data: any) { data.name.toUpperCase(); }

// ✅ 正确
function handle(data: unknown) {
  if (typeof data === 'object' && data !== null && 'name' in data) {
    // 类型已收窄
  }
}
```

### 响应式设计

管理面板需要在移动设备上完全可用（最小 320px 视口宽度）。

每个新的 UI 组件必须：
- 使用相对单位（rem、%、vw）而非固定像素宽度
- 在 320px、768px 和 1280px 断点上测试
- 触摸设备上的关键交互不依赖 hover 状态

---

## 测试

- **新功能需要添加测试**：覆盖主要路径和至少一个边界情况
- **修复 bug 时需要添加回归测试**：先写一个复现 bug 的测试（red-green-refactor）
- **Frontend 测试**：使用 Vitest，运行 `cd src/admin/ui && npm test`
- **Backend 测试**：使用 Rust 内置测试框架，运行 `cargo test -p inkforge`

---

## 提交 PR 前的检查清单

请确保你的 PR 满足以下所有条件：

**代码质量**
- [ ] 所有函数 ≤ 40 行
- [ ] 无魔法数字，常量已命名
- [ ] 后端验证所有输入
- [ ] 用户界面文本已国际化（中英文）

**技术规范**
- [ ] Rust：无 `unsafe` 块（除非有充分理由）
- [ ] Rust：`cargo clippy -- -D warnings` 通过
- [ ] TypeScript：无 `any` 类型
- [ ] 响应式：UI 在 320px 宽度下可用（如适用）

**测试与安全**
- [ ] 添加了相应的单元测试
- [ ] `cargo test -p inkforge` 通过
- [ ] `cd src/admin/ui && npm run build` 成功
- [ ] 没有提交密钥、tokens 或 AI 工具痕迹

**AI 辅助（如适用）**
- [ ] 在 PR 描述中说明了 AI 使用情况
- [ ] 理解并能解释提交的每一行代码

如果你不确定某项是否满足，可以在 PR 描述中说明，我们会帮助你完善。

---

## 常见修改原因

以下情况会导致 PR 被要求修改（不是拒绝，而是需要改进）：

| 问题 | 说明 | 如何修复 |
|------|------|----------|
| **移动端布局损坏** | 管理 UI 必须在 320px 宽屏幕上正常工作 | 使用响应式布局，在移动端测试 |
| **缺少国际化** | 硬编码的用户界面文本 | 使用 `t()` 函数和 i18n 系统 |
| **前端输入未验证** | 后端只依赖前端验证 | 后端添加独立的输入验证逻辑 |
| **函数过长** | 单个函数超过 40 行 | 拆分为多个小函数 |
| **命名不清晰** | 变量/函数名过于模糊 | 使用描述用途的完整名称 |
| **不必要的依赖** | 添加可以避免的 crate 或 npm 包 | 使用标准库或现有依赖 |

这些不是"硬性拒绝"，我们会指导你修改。

---

## 分支命名

分支名格式：`type/description`

`type` 可以是：

| Type | 用途 |
|------|------|
| `feat` | 新功能 |
| `fix` | Bug 修复 |
| `refactor` | 代码重构（不改变行为） |
| `docs` | 仅文档 |
| `chore` | 构建、CI、工具 |
| `test` | 添加或更新测试 |

**示例**：
- `feat/slug-editor`
- `fix/ime-cursor`
- `refactor/auth-middleware`

描述部分使用小写 kebab-case，简洁但清晰。

---

## Commit Message

中文或英文均可。保持中性，不要引用 AI 工具名称。

**格式**：

```
<type>: <简短描述>
```

**示例**：
- `feat: add slug collision detection in post editor`
- `fix: resolve IME composition cursor position`
- `feat: 添加文章 slug 冲突检测`
- `fix: 修复 IME 输入时光标位置问题`

主题行保持在 72 字符以内。

---

## AI 辅助贡献

你可以使用 AI 工具，但需要理解提交的每一行代码。

**如果使用了 AI**：

1. **在 PR 描述中说明**：
   ```markdown
   ## AI 辅助
   - [ ] 本 PR 不包含 AI 生成的代码
   - [x] 使用了 AI 辅助：[描述哪些部分及如何使用]
   ```

2. **理解你的代码**：你对提交的每一行负责。如果 reviewer 要求你解释某个部分，你应该能够做到。

3. **清理 AI 痕迹**：提交前移除任何 AI 生成的注释、虚构的引用或工具特定的标记。

4. **仔细审查**：AI 生成的代码在错误处理、边界情况和安全边界方面容易有细微的 bug，请格外留意。

---

## 安全

- **不要提交密钥**：API keys、tokens、密码和私钥应该放在环境变量或 gitignored 的配置文件中
- **不要提交 AI 辅助痕迹**：Commit message、代码注释和文档中不要引用具体的 AI 工具、模型或 agent 名称
- **不要记录敏感数据**：包含密码、tokens 或 PII 的请求体不应出现在日志中

---

## 需要帮助？

遇到问题？可以：

- 在 [Issues](https://github.com/wenaryHY/inkforge/issues) 提问
- 在 [Discussions](https://github.com/wenaryHY/inkforge/discussions) 讨论想法
- 查看现有代码寻找类似例子

---

## 许可证

贡献 InkForge 即表示你同意你的贡献将以 [GNU Affero General Public License v3.0](LICENSE) (AGPLv3) 授权。

---

**感谢你的贡献！**

每个贡献都让 InkForge 变得更好。我们期待与你一起构建优秀的 CMS。

---

## English Version

Thank you for your interest in InkForge! We welcome all forms of contributions.

**Before submitting a PR**, please take 5 minutes to read this document. It will help your contribution get merged smoothly.

### First-Time Contributors

First time contributing to open source? You can start with these tasks:

- Fix typos or unclear explanations in documentation
- Add unit tests
- Improve i18n translations
- Fix issues labeled `good first issue`

Not sure if a change is appropriate? Open an issue first to discuss.

---

## Code Quality Standards

InkForge is a project that values code quality. We will not lower standards for the sake of fast merges.

This means your PR needs to:

- Pass all automated checks (rustfmt, clippy, tests)
- Follow code conventions (function length, naming, error handling)
- Have adequate test coverage

If this is your first contribution, don't worry — we will provide detailed feedback during review to help you meet the standards.

---

## Development Environment

### Requirements

- Rust 1.75+
- Node.js 22+
- SQLite 3

### Setup

```bash
git clone https://github.com/wenaryHY/inkforge.git
cd inkforge
cd src/admin/ui && npm ci && cd -
```

### Start Development Server

```bash
# Terminal 1: Rust backend
cargo run

# Terminal 2: Admin panel Vite dev server
cd src/admin/ui && npm run dev
```

Or use the combined command from the project root:

```bash
npm run dev
```

### Production Build

```bash
cargo build --release -p inkforge
```

The admin UI production build is automatically embedded during `cargo build`.

### Run Tests

```bash
# Backend tests
cargo test -p inkforge

# Frontend type checking + build
cd src/admin/ui && npm run build
```

Combined command:

```bash
cargo test -p inkforge && cd src/admin/ui && npm run build
```

### Lint

```bash
# Rust
cargo clippy -- -D warnings

# Frontend
cd src/admin/ui && npm run lint
```

---

## Code Conventions

The following conventions are **mandatory**. PRs that do not comply will be asked to revise.

### Function Length

- **Requirement**: Every function ≤ 40 lines (excluding comments and blank lines)
- **Reason**: Short functions are easier to test, debug, and understand
- **How**: When exceeding 40 lines, consider splitting into multiple smaller functions

### Naming Principles

Priority: **Correctness > Precision > Consistency**

Names must accurately describe their purpose, return value, and side effects. Long names are acceptable; brevity should not compromise correctness.

✅ **Good Examples**:
- `SESSION_STORAGE_KEY_FOR_PREVIEW_PARAMETERS_PASSED_TO_NEW_TAB` — Fully describes purpose
- `validateThemeSlugIsInstalledAndSafeForPreviewRendering` — Precisely describes validation logic
- `calculatePopoverPositionRelativeToFabButtonRect` — Pure function doesn't start with `use`

❌ **Bad Examples**:
- `usePopoverPosition` — Pure computation function shouldn't use React Hook prefix `use`
- `data` / `item` / `result` — No specific meaning

### No Magic Numbers

- **Requirement**: All numeric constants must be named
- **Bad Example**: `if attempts > 5`
- **Good Example**:
  ```rust
  const MAX_LOGIN_ATTEMPTS: i32 = 5;
  if attempts > MAX_LOGIN_ATTEMPTS { ... }
  ```

### Internationalization

- **Requirement**: All user-facing text must support both English and Chinese through the i18n system
- **Not Allowed**: Hardcoded English or Chinese strings

```typescript
// ❌ Wrong
<h1>Welcome</h1>

// ✅ Right
<h1>{t('welcome.title')}</h1>
```

When adding new translatable strings:
1. Add English key in base i18n definition
2. Add Chinese translation
3. Use the key in components

### Backend Input Validation

The backend needs to independently validate, sanitize, and authorize all received data. Frontend validation is only for user experience, not a security boundary.

Every endpoint must check:
- Authentication and authorization
- Input format and constraints
- Resource ownership

---

## Rust Guidelines

### Zero-Cost Abstraction Principle

When implementing swappable components like cross-database, cross-storage-backend, cross-protocol:

- **Must use abstraction** (trait + generics), prohibit hardcoded concrete types
- **Abstraction should not introduce runtime overhead** — Prefer compile-time polymorphism (generics), avoid `Box<dyn Trait>` unless necessary

✅ **Good Example**:
```rust
// Generic parameter, compile-time monomorphization, zero overhead
async fn list_posts<DB: sqlx::Database>(pool: &sqlx::Pool<DB>) -> Vec<Post> { ... }
```

❌ **Bad Example**:
```rust
// Hardcoded concrete type — need to change 151 places when switching databases
async fn list_posts(pool: &SqlitePool) -> Vec<Post> { ... }
```

### Avoid `unsafe`

InkForge is a web application serving untrusted clients and typically doesn't need `unsafe` code. If you believe it's truly necessary, please explain in detail in your PR.

### Clippy Check

Run `cargo clippy -- -D warnings` before submitting. We require PRs not to introduce new Clippy warnings.

---

## Frontend Guidelines

### No `any`

Use `unknown` and perform type narrowing.

```typescript
// ❌ Wrong
function handle(data: any) { data.name.toUpperCase(); }

// ✅ Right
function handle(data: unknown) {
  if (typeof data === 'object' && data !== null && 'name' in data) {
    // Type narrowed
  }
}
```

### Responsive Design

The admin panel needs to be fully usable on mobile devices (minimum 320px viewport width).

Every new UI component must:
- Use relative units (rem, %, vw) rather than fixed pixel widths
- Test at 320px, 768px, and 1280px breakpoints
- Not depend on hover states for critical interactions on touch devices

---

## Testing

- **New features need tests**: Cover main paths and at least one edge case
- **Bug fixes need regression tests**: Write a test that reproduces the bug first (red-green-refactor)
- **Frontend tests**: Use Vitest, run `cd src/admin/ui && npm test`
- **Backend tests**: Use Rust's built-in test framework, run `cargo test -p inkforge`

---

## Pre-PR Checklist

Please ensure your PR meets all of the following conditions:

**Code Quality**
- [ ] All functions ≤ 40 lines
- [ ] No magic numbers, constants are named
- [ ] Backend validates all inputs
- [ ] User interface text is internationalized (English & Chinese)

**Technical Standards**
- [ ] Rust: No `unsafe` blocks (unless with good reason)
- [ ] Rust: `cargo clippy -- -D warnings` passes
- [ ] TypeScript: No `any` types
- [ ] Responsive: UI works at 320px width (if applicable)

**Testing & Security**
- [ ] Added relevant unit tests
- [ ] `cargo test -p inkforge` passes
- [ ] `cd src/admin/ui && npm run build` succeeds
- [ ] No committed keys, tokens, or AI tool traces

**AI Assistance (if applicable)**
- [ ] Disclosed AI usage in PR description
- [ ] Understand and can explain every line of submitted code

If you're unsure whether something meets the requirements, mention it in your PR description and we'll help you improve it.

---

## Common Revision Reasons

The following situations will result in PRs being asked to revise (not rejection, but need improvement):

| Issue | Description | How to Fix |
|-------|-------------|------------|
| **Mobile layout broken** | Admin UI must work on 320px screens | Use responsive layout, test on mobile |
| **Missing i18n** | Hardcoded user interface text | Use `t()` function and i18n system |
| **Frontend input not validated** | Backend only relies on frontend validation | Add independent input validation logic on backend |
| **Function too long** | Single function exceeds 40 lines | Split into multiple smaller functions |
| **Unclear naming** | Variable/function names too vague | Use full names that describe purpose |
| **Unnecessary dependencies** | Adding avoidable crates or npm packages | Use standard library or existing dependencies |

These are not "hard rejections" — we will guide you through the revisions.

---

## Branch Naming

Branch name format: `type/description`

`type` can be:

| Type | Purpose |
|------|---------|
| `feat` | New features |
| `fix` | Bug fixes |
| `refactor` | Code refactoring (no behavior change) |
| `docs` | Documentation only |
| `chore` | Build, CI, tools |
| `test` | Add or update tests |

**Examples**:
- `feat/slug-editor`
- `fix/ime-cursor`
- `refactor/auth-middleware`

Description part uses lowercase kebab-case, concise but clear.

---

## Commit Messages

English or Chinese is acceptable. Keep neutral, don't reference AI tool names.

**Format**:

```
<type>: <brief description>
```

**Examples**:
- `feat: add slug collision detection in post editor`
- `fix: resolve IME composition cursor position`
- `feat: 添加文章 slug 冲突检测`
- `fix: 修复 IME 输入时光标位置问题`

Keep subject line within 72 characters.

---

## AI-Assisted Contributions

You can use AI tools, but you need to understand every line of code you submit.

**If you used AI**:

1. **Disclose in PR description**:
   ```markdown
   ## AI Assistance
   - [ ] This PR does not contain AI-generated code
   - [x] Used AI assistance: [describe which parts and how used]
   ```

2. **Understand your code**: You are responsible for every line submitted. If reviewers ask you to explain a section, you should be able to.

3. **Clean up AI traces**: Remove any AI-generated comments, fictional references, or tool-specific markers before submission.

4. **Review carefully**: AI-generated code tends to have subtle bugs in error handling, edge cases, and security boundaries. Pay extra attention to these areas.

---

## Security

- **Don't commit keys**: API keys, tokens, passwords, and private keys should be in environment variables or gitignored config files
- **Don't commit AI assistance traces**: Don't reference specific AI tools, models, or agent names in commit messages, code comments, or documentation
- **Don't log sensitive data**: Request bodies containing passwords, tokens, or PII should not appear in logs

---

## Need Help?

Having trouble? You can:

- Ask in [Issues](https://github.com/wenaryHY/inkforge/issues)
- Discuss ideas in [Discussions](https://github.com/wenaryHY/inkforge/discussions)
- Look at existing code for similar examples

---

## License

By contributing to InkForge, you agree that your contributions will be licensed under the [GNU Affero General Public License v3.0](LICENSE) (AGPLv3).

---

**Thank you for your contribution!**

Every contribution makes InkForge better. We look forward to building an excellent CMS together with you.
