# Contributing to InkForge

We welcome contributors of any experience level. We do not lower code standards.
欢迎任何经验水平的贡献者，但不会降低代码标准。

InkForge is a Rust CMS that compiles to a single static binary, with a React admin panel embedded inside. We care about correctness, performance, and code clarity. This document tells you how to contribute in a way that respects the project's values — and saves you from having your PR rejected for preventable reasons.

---

## Before You Start

- **Major features require an issue first.** Before writing code for a new feature, refactor, or architecture change, open a GitHub Issue describing your intent. This lets maintainers give early feedback and prevents wasted effort.
- **Small fixes can skip the issue.** Typos, minor bug fixes, translation corrections — just open a PR directly.
- **Read the Coding Principles section below.** It contains project-specific rules that every contributor must follow, including naming conventions, function length limits, and the interface-first workflow.

---

## Development Workflow

### Prerequisites

- Rust 1.75+
- Node.js 22+
- SQLite 3

### Setup

```bash
git clone https://github.com/wenaryHY/inkforge.git
cd inkforge
cd src/admin/ui && npm ci && cd -
```

### Run the dev server

```bash
# Terminal 1: Rust backend
cargo run

# Terminal 2: Vite dev server for the admin UI
cd src/admin/ui && npm run dev
```

Or use the combined command from the project root:

```bash
npm run dev
```

### Build for production

```bash
cargo build --release -p inkforge
```

The production build of the admin UI is automatically embedded during `cargo build`. You do not need to build the frontend separately for production.

### Run tests

```bash
# Backend tests
cargo test -p inkforge

# Frontend type-check + build (serves as compile-time verification)
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

## Coding Principles

### Correctness > Accuracy > Consistency (Naming)

Names must be correct first, precise second, and consistent third. We will accept a very long name if it is the most accurate description of what something does. We will reject a short, "clean" name if it misleads.

**Good:**
- `SESSION_STORAGE_KEY_FOR_PREVIEW_PARAMETERS_PASSED_TO_NEW_TAB` — fully describes its purpose
- `calculatePopoverPositionRelativeToFabButtonRect` — pure function, not a React Hook

**Bad:**
- `usePopoverPosition` — a pure calculation function should never use the `use` prefix reserved for React Hooks
- `data`, `item`, `result` — no specific meaning

### Functions: Keep Them Short (≤40 lines)

Every function should do one thing. If your function description contains the word "and", consider splitting it. We enforce a hard limit of 40 lines per function. Functions exceeding this limit will be flagged in review.

Rationale: short functions are easier to test, review, and reason about. They also make stack traces more useful.

### No Magic Numbers

Every hardcoded numeric literal must be extracted into a named constant. This includes timeout durations, buffer sizes, retry counts, cache TTLs, pixel values, and everything in between.

```rust
// Bad
let result = retry_operation(3, Duration::from_secs(5));

// Good
const MAX_RETRY_COUNT: u32 = 3;
const RETRY_BACKOFF_SECONDS: u64 = 5;
let result = retry_operation(MAX_RETRY_COUNT, Duration::from_secs(RETRY_BACKOFF_SECONDS));
```

### Never Trust the Frontend

The backend must validate, sanitize, and authorize every piece of data it receives. The frontend is a convenience layer, not a security boundary. An attacker can and will bypass your React forms and send raw HTTP requests. Every endpoint must independently verify:

- Authentication and authorization
- Input format and constraints
- Ownership of referenced resources

### Internationalization Is Mandatory

All user-facing strings must support both English and Chinese (zh-CN). Never hardcode display text. Use the i18n system:

```typescript
// Bad
<h1>Welcome</h1>

// Good
<h1>{t('welcome.title')}</h1>
```

When you add a new translatable string:
1. Add the English key to the base i18n definitions
2. Add the Chinese translation
3. Use the key in your component

---

## Rust Guidelines

### Prefer Zero-Cost Abstractions

Use generics and traits over runtime dispatch. The compiler monomorphizes generics into concrete types, producing the same machine code as if you had written the concrete type by hand. There is no performance penalty.

```rust
// Preferred: generic, monomorphized at compile time
async fn list_posts<DB: sqlx::Database>(pool: &sqlx::Pool<DB>) -> Vec<Post> { ... }

// Avoid: hardcoded to one database
async fn list_posts(pool: &SqlitePool) -> Vec<Post> { ... }
```

Before adding a new storage backend, API client, or protocol implementation, ask: "Will this ever be swapped out?" If yes, abstract it behind a trait.

### No `unsafe`

InkForge is a web application serving untrusted clients over the network. There is no justification for `unsafe` code in this context. Any PR containing `unsafe` blocks will be rejected unless accompanied by a documented, maintainer-approved justification that explains why a safe alternative is impossible.

### Avoid `Box<dyn Trait>`

Prefer static dispatch via generics. `Box<dyn Trait>` introduces heap allocation and a vtable lookup. Use it only when the set of concrete types is genuinely not known at compile time (e.g., plugin registration at runtime). Even then, consider `enum`-based dispatch first.

### Clippy Should Be Clean

Run `cargo clippy -- -D warnings` before pushing. Your PR must introduce zero new Clippy warnings. We treat Clippy warnings as compilation errors.

---

## Frontend Guidelines

### No `any`

Use `unknown` and narrow the type. TypeScript's `any` disables the type checker — it defeats the purpose of using TypeScript. If you genuinely cannot determine a type, use `unknown` and perform runtime checks before accessing properties.

```typescript
// Bad
function handle(data: any) { data.name.toUpperCase(); }

// Good
function handle(data: unknown) {
  if (typeof data === 'object' && data !== null && 'name' in data) {
    // type is narrowed
  }
}
```

### Responsive First

The admin panel must be fully usable on mobile devices (minimum 320px viewport width). Every new UI component must:

- Use relative units (rem, %, vw) rather than fixed pixel widths
- Be tested at 320px, 768px, and 1280px breakpoints
- Not rely on hover states for critical interactions on touch devices

---

## Testing

- **Write tests for new features.** If you add a function, add a test that exercises its happy path and at least one edge case.
- **Fix bugs with regression tests.** A bug report should result in a test that reproduces the bug before the fix is written (red-green-refactor).
- **Frontend tests** use Vitest. Run with `cd src/admin/ui && npm test`.
- **Backend tests** use Rust's built-in test framework. Run with `cargo test -p inkforge`.
- **E2E tests** live in `e2e/`. These are optional for most PRs but required for changes to authentication flows, setup wizards, or critical user journeys.

---

## Security

- **Never commit secrets.** API keys, tokens, passwords, and private keys belong in environment variables or gitignored configuration files. If you accidentally commit a secret, rotate it immediately and notify a maintainer.
- **Never commit AI-assistance traces.** Commit messages, code comments, and documentation must not reference specific AI tools, models, or agent names.
- **Do not log sensitive data.** Request bodies containing passwords, tokens, or PII must never appear in logs, even at debug level.

---

## Branch Naming

Branch names follow the format:

```
type/description
```

Where `type` is one of:

| Type | Purpose |
|------|---------|
| `feat` | New feature |
| `fix` | Bug fix |
| `refactor` | Code restructuring without behavior change |
| `docs` | Documentation only |
| `chore` | Build, CI, tooling |
| `test` | Adding or updating tests |

**Examples:**
- `feat/slug-editor`
- `fix/ime-cursor`
- `refactor/auth-middleware`
- `docs/api-usage`

Use lowercase kebab-case for the description. Be descriptive but concise.

---

## Commit Messages

English is preferred, but Chinese is acceptable.

**Format:**

```
<type>: <short description>
```

**Examples:**
- `feat: add slug collision detection in post editor`
- `fix: resolve IME composition cursor position`
- `refactor: extract auth middleware into separate module`
- `docs: update API error code reference`

Keep the subject line under 72 characters. Use the body for additional context when needed.

---

## AI-Assisted Contributions

You may use AI tools. You must understand every line you submit.
You must disclose AI-assisted sections in the PR description.
Code will never be rejected solely because AI was used.

**Specific requirements when AI was involved:**

1. **Disclosure:** In your PR description, add a section:
   ```
   ## AI Assistance
   - [ ] This PR contains no AI-generated code
   - [x] AI was used for: [describe which parts and how]
   ```

2. **Understanding:** You are responsible for every line you push. If a reviewer asks you to explain a section during review, you must be able to do so.

3. **No traces:** Remove any AI-generated comments, hallucinated references, or tool-specific markers from your code before committing. This includes comments like "Generated by X" or placeholder text left by AI tools.

4. **Review readiness:** AI-generated code tends to have subtle bugs around error handling, edge cases, and security boundaries. Review AI output extra carefully in these areas.

---

## Pull Request Checklist

Before marking a PR as ready for review, confirm:

- [ ] I have read the Coding Principles section
- [ ] All new functions are ≤ 40 lines
- [ ] No magic numbers — all constants are named
- [ ] Backend validates all input — I do not trust the frontend
- [ ] User-facing strings use the i18n system (English + Chinese translations)
- [ ] No `unsafe` blocks (Rust)
- [ ] No `any` types (TypeScript)
- [ ] `cargo clippy` passes with zero warnings
- [ ] `cargo test -p inkforge` passes
- [ ] `cd src/admin/ui && npm run build` succeeds
- [ ] UI is tested at 320px, 768px, and 1280px widths (if applicable)
- [ ] Tests are added for new behavior
- [ ] Regression test is added for the bug being fixed
- [ ] No secrets, tokens, or AI traces in the diff
- [ ] AI assistance is disclosed in the PR description (if applicable)

---

## What Gets Rejected

These are the most common reasons PRs get rejected. Checking this list before you open a PR saves everyone time.

| Reason | Explanation |
|--------|-------------|
| **Unclear naming** | If a variable/function name does not describe what it does, it needs to be renamed. Follow: correctness > accuracy > consistency. |
| **Unnecessary dependencies** | Adding a crate or npm package must be justified. Does the standard library or an existing dependency already provide this? Could you implement the needed subset in under 50 lines? |
| **Broken mobile layout** | The admin UI must work on 320px-wide screens. If your change breaks mobile, it will not be merged. |
| **Excessive configuration** | InkForge values sensible defaults. Do not add config options for things that work for 95% of users out of the box. Every new config flag is a maintenance burden. |
| **Missing i18n** | Hardcoded user-facing strings are a hard reject. Everything must go through the i18n system with both English and Chinese translations. |
| **Functions over 40 lines** | Break it up. If you cannot describe your function's purpose without the word "and", it does too much. |
| **Trusting frontend input** | The backend must independently validate everything. No exceptions. |
| **Silent failures** | Every fallible operation must either return a `Result` (Rust) or be explicitly handled. Swallowed errors are bugs. |

---

## License

By contributing to InkForge, you agree that your contributions will be licensed under the [GNU Affero General Public License v3.0](LICENSE) (AGPLv3).
