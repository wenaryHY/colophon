# Contributing to Colophon

**[English](#english)** | **[中文](#zh)**

Thank you for your interest in Colophon! We welcome all forms of contributions.
感谢你对 Colophon 的关注！我们欢迎各种形式的贡献。

---

## Good First Issues

Look for issues labeled `good first issue` in the [issue tracker](https://github.com/wenaryHY/colophon/issues).

Good places to start:
- Fix a typo or improve an error message
- Add a missing i18n translation
- Write a test for an untested function
- Improve a doc comment

Not sure if a change is appropriate? Open an issue first to discuss.

---

## Before You Start

- Rust 1.75+
- Node.js 22+
- SQLite 3

```bash
git clone https://github.com/wenaryHY/colophon.git
cd colophon
cd src/admin/ui && npm ci && cd -
```

---

## Development Workflow

```bash
# Start backend
cargo run

# Start admin panel (separate terminal)
cd src/admin/ui && npm run dev

# Or combined
npm run dev
```

**Production build:**

```bash
cargo build --release -p colophon
```

The admin UI is automatically embedded during `cargo build`.

**Testing:**

```bash
# Backend
cargo test -p colophon

# Frontend type check + build
cd src/admin/ui && npm run build
```

**Lint:**

```bash
# Rust
cargo clippy -- -D warnings

# Frontend
cd src/admin/ui && npm run lint
```

---

## Coding Principles

### Function Length

Every function must not exceed 40 lines (excluding comments and blank lines). Short functions are easier to test, debug, and understand. Split longer functions into smaller ones.

### Naming

Priority: **Correctness > Precision > Consistency.** Names must accurately describe purpose, return value, and side effects. Long names are acceptable; brevity must not compromise correctness. Good: `validateThemeSlugIsInstalledAndSafeForPreviewRendering`. Bad: `data`, `item`, `result`.

### No Magic Numbers

All numeric constants must be named. `if attempts > 5` is unacceptable; use `const MAX_LOGIN_ATTEMPTS: i32 = 5;`.

### Internationalization

All user-facing text must support both English and Chinese via the i18n system. Never hardcode user-visible strings.

### Backend Input Validation

The backend must independently validate, sanitize, and authorize all incoming data. Frontend validation is UX only, not a security boundary. Every endpoint checks authentication, input constraints, and resource ownership.

---

## Rust Guidelines

- **Zero-cost abstraction**: Use traits + generics for swappable components (databases, storage, protocols). Prefer compile-time polymorphism; avoid `Box<dyn Trait>` unless necessary.
- **No `unsafe`**: Colophon serves untrusted clients. If you believe `unsafe` is needed, explain in your PR.
- **Clippy clean**: `cargo clippy -- -D warnings` must pass before submitting.

---

## Frontend Guidelines

- **No `any`**: Use `unknown` with type narrowing.
- **Responsive design**: Admin panel must work at 320px viewport width. Use relative units, test at 320px / 768px / 1280px breakpoints. Don't rely on hover for critical touch interactions.

---

## Testing

- New features must include tests covering the main path and at least one edge case.
- Bug fixes must include a regression test that reproduces the bug (red-green-refactor).
- Backend: `cargo test -p colophon`. Frontend: `cd src/admin/ui && npm test`.

---

## Commit Messages

English or Chinese accepted. Keep neutral — no AI tool names. Subject line within 72 characters.

```
<type>: <brief description>
```

Examples: `feat: add slug collision detection`, `fix: 修复 IME 输入时光标位置问题`.

---

## Branch Naming

Format: `type/description` (lowercase kebab-case).

| Type | Purpose |
|------|---------|
| `feat` | New features |
| `fix` | Bug fixes |
| `refactor` | Code refactoring |
| `docs` | Documentation only |
| `chore` | Build, CI, tools |
| `test` | Add or update tests |

Examples: `feat/slug-editor`, `fix/ime-cursor`, `refactor/auth-middleware`.

---

## AI Policy

You may use AI tools, but you must understand every line you submit.

**If AI was used:**
1. Disclose in your PR description which parts and how.
2. Be ready to explain any part of your code when asked.
3. Remove all AI-generated comments, fictional references, or tool-specific markers before submission.
4. Review carefully — AI-generated code often has subtle bugs in error handling and edge cases.

---

## Pull Request Checklist

**Code Quality**
- [ ] All functions ≤ 40 lines
- [ ] No magic numbers, constants are named
- [ ] Backend validates all inputs
- [ ] User-visible text is internationalized (English & Chinese)

**Technical Standards**
- [ ] Rust: no `unsafe` (unless justified); `cargo clippy -- -D warnings` passes
- [ ] TypeScript: no `any` types
- [ ] UI works at 320px width (if applicable)

**Testing**
- [ ] Relevant tests added; `cargo test -p colophon` passes
- [ ] `cd src/admin/ui && npm run build` succeeds
- [ ] No committed keys, tokens, or AI tool traces

---

## What Gets Rejected

| Issue | How to fix |
|-------|------------|
| **Missing i18n** | Use `t()` function and the i18n system |
| **Backend trusts frontend** | Add independent input validation on the backend |
| **Function too long** | Split into smaller functions |
| **Unclear naming** | Use full names that describe purpose |
| **Unnecessary dependencies** | Use standard library or existing dependencies |
| **Mobile layout broken** | Use responsive layout, test at 320px |

These are not hard rejections — we will guide you through revisions.

---

## Need Help?

- Ask in [Issues](https://github.com/wenaryHY/colophon/issues)
- Discuss ideas in [Discussions](https://github.com/wenaryHY/colophon/discussions)
- Look at existing code for similar examples

---

## License

By contributing, you agree that your contributions will be licensed under the [GNU Affero General Public License v3.0](LICENSE) (AGPLv3).

---

**Every contribution makes Colophon better. Thank you!**
