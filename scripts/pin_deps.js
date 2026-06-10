const fs = require('fs');
const path = require('path');

const ROOT_PKG = 'd:/colophon/package.json';
const UI_PKG = 'd:/colophon/src/admin/ui/package.json';
const CARGO_TOML = 'd:/colophon/Cargo.toml';
const TAURI_TOML = 'd:/colophon/src-tauri/Cargo.toml';
const VERSIONS_MD = 'd:/colophon/memories/PACKAGE_VERSIONS.md';

function stripCaret(obj) {
  if (!obj) return;
  for (const key in obj) {
    obj[key] = obj[key].replace(/^[~^]/, '');
  }
}

// 1. Update Root Package JSON
let rootPkg = JSON.parse(fs.readFileSync(ROOT_PKG, 'utf-8'));
stripCaret(rootPkg.dependencies);
stripCaret(rootPkg.devDependencies);
fs.writeFileSync(ROOT_PKG, JSON.stringify(rootPkg, null, 2) + '\n');

// 2. Update UI Package JSON
let uiPkg = JSON.parse(fs.readFileSync(UI_PKG, 'utf-8'));
stripCaret(uiPkg.dependencies);
stripCaret(uiPkg.devDependencies);
fs.writeFileSync(UI_PKG, JSON.stringify(uiPkg, null, 2) + '\n');

// 3. Update Root Cargo.toml
const rootCargoDeps = {
  ammonia: "4.1.2", anyhow: "1.0.102", argon2: "0.5.3", axum: "0.7.9",
  "axum-extra": "0.9.6", bytes: "1.11.1", chrono: "0.4.44", config: "0.14.1",
  "futures-util": "0.3.32", hex: "0.4.3", jsonwebtoken: "9.3.1", mime_guess: "2.0.5",
  minijinja: "2.19.0", once_cell: "1.21.4", "pulldown-cmark": "0.11.3",
  rand_core: "0.6.4", serde: "1.0.228", serde_json: "1.0.149", sha2: "0.10.9",
  slug: "0.1.6", sqlx: "0.7.4", thiserror: "1.0.69", tokio: "1.51.0",
  "tokio-cron-scheduler": "0.9.4", toml: "0.8.2", tower: "0.4.13",
  "tower-http": "0.5.2", tracing: "0.1.44", "tracing-subscriber": "0.3.23",
  url: "2.5.8", uuid: "1.23.0", zip: "2.4.2"
};

let cargoToml = fs.readFileSync(CARGO_TOML, 'utf-8');
for (const [dep, ver] of Object.entries(rootCargoDeps)) {
  const r1 = new RegExp(`^${dep}\\s*=\\s*\\{\\s*version\\s*=\\s*"[^"]+"(.*?)\\}$`, 'm');
  cargoToml = cargoToml.replace(r1, `${dep} = { version = "=${ver}"$1}`);
  
  const r2 = new RegExp(`^${dep}\\s*=\\s*"[^"]+"$`, 'm');
  cargoToml = cargoToml.replace(r2, `${dep} = "=${ver}"`);
}
fs.writeFileSync(CARGO_TOML, cargoToml);

// 4. Update Tauri Cargo.toml
const tauriDeps = {
  tauri: "2.10.3", "tauri-plugin-shell": "2.3.5", reqwest: "0.12.28",
  tokio: "1.51.0", serde: "1.0.228", serde_json: "1.0.149", tracing: "0.1.44"
};

let tauriToml = fs.readFileSync(TAURI_TOML, 'utf-8');
for (const [dep, ver] of Object.entries(tauriDeps)) {
  const r1 = new RegExp(`^${dep}\\s*=\\s*\\{\\s*version\\s*=\\s*"[^"]+"(.*?)\\}$`, 'm');
  tauriToml = tauriToml.replace(r1, `${dep} = { version = "=${ver}"$1}`);
  
  const r2 = new RegExp(`^${dep}\\s*=\\s*"[^"]+"$`, 'm');
  tauriToml = tauriToml.replace(r2, `${dep} = "=${ver}"`);
}
tauriToml = tauriToml.replace(/^tauri-build\s*=\s*\{\s*version\s*=\s*"[^"]+"(.*?)\}/m, `tauri-build = { version = "=2.5.6"$1}`);
fs.writeFileSync(TAURI_TOML, tauriToml);

// 5. Generate Markdown
let md = `# Package Versions
> Generated automatically to pin exact versions of all dependencies across the stack.
> Rules: Future additions MUST use exact versions (e.g., \`=x.y.z\` for Cargo, \`x.y.z\` without prefix for npm).

## Root Node.js Dependencies (package.json)
\`\`\`json
${JSON.stringify({ dependencies: rootPkg.dependencies, devDependencies: rootPkg.devDependencies }, null, 2)}
\`\`\`

## Frontend UI Dependencies (src/admin/ui/package.json)
\`\`\`json
${JSON.stringify({ dependencies: uiPkg.dependencies, devDependencies: uiPkg.devDependencies }, null, 2)}
\`\`\`

## Backend Rust Dependencies (Cargo.toml)
\`\`\`json
${JSON.stringify(rootCargoDeps, null, 2)}
\`\`\`

## Tauri Desktop Dependencies (src-tauri/Cargo.toml)
\`\`\`json
${JSON.stringify({ ...tauriDeps, "tauri-build": "2.5.6" }, null, 2)}
\`\`\`
`;

fs.writeFileSync(VERSIONS_MD, md);
console.log("Versions pinned and memories/PACKAGE_VERSIONS.md updated successfully!");
