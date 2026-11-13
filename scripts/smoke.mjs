import { spawn } from 'node:child_process';
import { readFile, stat } from 'node:fs/promises';
import { resolve } from 'node:path';

const distIndexPath = resolve('dist-renderer/index.html');
const cargoManifestPath = resolve('src-tauri/Cargo.toml');

function runProcess(command, args, { env = process.env, timeoutMs = 300_000 } = {}) {
  return new Promise((resolvePromise, rejectPromise) => {
    const child = spawn(command, args, {
      cwd: process.cwd(),
      env,
      stdio: ['ignore', 'pipe', 'pipe'],
      windowsHide: true,
    });

    let stdout = '';
    let stderr = '';
    let timedOut = false;

    child.stdout.setEncoding('utf8');
    child.stderr.setEncoding('utf8');
    child.stdout.on('data', (chunk) => {
      stdout += chunk;
    });
    child.stderr.on('data', (chunk) => {
      stderr += chunk;
    });

    const timeout = setTimeout(() => {
      timedOut = true;
      child.kill();
    }, timeoutMs);

    child.on('error', (error) => {
      clearTimeout(timeout);
      rejectPromise(new Error(`Failed to start "${command}": ${error.message}`));
    });

    child.on('close', (code) => {
      clearTimeout(timeout);
      if (timedOut) {
        rejectPromise(
          new Error(
            `Command "${command} ${args.join(' ')}" timed out after ${timeoutMs}ms.\n${stderr.trim()}`,
          ),
        );
        return;
      }
      if (code !== 0) {
        const details = [stdout.trim(), stderr.trim()].filter(Boolean).join('\n');
        rejectPromise(
          new Error(
            `Command "${command} ${args.join(' ')}" exited with code ${code}.${details ? `\n${details}` : ''}`,
          ),
        );
        return;
      }
      resolvePromise({ stdout, stderr });
    });
  });
}

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
    const [npmCmd, npmArgs] =
      process.platform === 'win32'
        ? ['cmd.exe', ['/d', '/c', 'npm.cmd', 'run', 'build:web']]
        : ['npm', ['run', 'build:web']];
    await runProcess(npmCmd, npmArgs);
  }

  if (!(await hasNonEmptyFile(distIndexPath))) {
    throw new Error(`Renderer build output missing or empty at ${distIndexPath}`);
  }
}

async function runRustSmokeTests() {
  await runProcess('cargo', ['test', '--manifest-path', cargoManifestPath, '--quiet'], {
    env: {
      ...process.env,
      AUTHAPP_SMOKE_TEST: '1',
    },
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
