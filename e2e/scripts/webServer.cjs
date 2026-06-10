const { spawn } = require('child_process');
const path = require('path');

const rootDir = path.resolve(__dirname, '..', '..');
const uiDir = path.join(rootDir, 'src', 'admin', 'ui');

function run(cmd, args, options = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(cmd, args, {
      shell: true,
      stdio: 'inherit',
      ...options,
    });

    child.on('exit', (code) => {
      if (code === 0) {
        resolve();
      } else {
        reject(new Error(`${cmd} exited with code ${code}`));
      }
    });

    child.on('error', reject);
  });
}

async function main() {
  await run('npm', ['run', 'build'], { cwd: uiDir });

  const env = {
    ...process.env,
    COLOPHON__SERVER__PORT: '2002',
    COLOPHON__DATABASE__URL: 'sqlite::memory:',
    COLOPHON__STORAGE__UPLOAD_DIR: 'target_tmp_e2e_uploads',
    COLOPHON__THEME__THEME_DIR: 'target_tmp_e2e_themes',
    COLOPHON__RUNTIME__MODE: 'development',
  };

  const server = spawn('cargo', ['run'], {
    cwd: rootDir,
    env,
    shell: true,
    stdio: 'inherit',
  });

  const shutdown = () => {
    if (!server.killed) {
      server.kill();
    }
  };

  process.on('SIGINT', shutdown);
  process.on('SIGTERM', shutdown);
  process.on('exit', shutdown);

  server.on('exit', (code) => {
    process.exit(code ?? 0);
  });

  server.on('error', (err) => {
    console.error('[e2e-webserver] failed to start cargo run:', err);
    process.exit(1);
  });
}

main().catch((err) => {
  console.error('[e2e-webserver] bootstrap failed:', err);
  process.exit(1);
});
