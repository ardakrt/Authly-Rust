import { execFile } from 'node:child_process';
import { readFile, stat } from 'node:fs/promises';
import { promisify } from 'node:util';

const execFileAsync = promisify(execFile);
const distIndexPath = 'dist-renderer/index.html';

async function hasNonEmptyFile(filePath) {
  try {
    const info = await stat(filePath);
    if (!info.isFile() || info.size === 0) {
      return false;
    }
    const content = await readFile(filePath, 'utf8');
    return content.trim().length > 0;
  } catch {
    return false;
  }
}

async function ensureWebBuild() {
  if (!(await hasNonEmptyFile(distIndexPath))) {
    const { build } = await import('vite');
    await build();
  }

  if (!(await hasNonEmptyFile(distIndexPath))) {
    throw new Error(`Renderer build output missing or empty at ${distIndexPath}`);
  }
}

async function runRustSmokeTests() {
  await execFileAsync('cargo', ['test', '--manifest-path', 'src-tauri/Cargo.toml', '--quiet'], {
    env: {
      ...process.env,
      AUTHAPP_SMOKE_TEST: '1',
    },
    windowsHide: true,
    timeout: 300_000,
    maxBuffer: 10 * 1024 * 1024,
  });
}

try {
  await ensureWebBuild();
  await runRustSmokeTests();
  console.log('AUTHAPP_SMOKE_READY');
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  console.error(`Tauri smoke check failed: ${message}`);
  process.exitCode = 1;
}
