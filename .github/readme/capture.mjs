/* global process, console, URL, window, document, localStorage */
// Render the real UI with synthetic IPC responses. Never reads a vault or creates TOTP secrets.
import { createServer } from 'vite';
import react from '@vitejs/plugin-react';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import fs from 'node:fs/promises';
import assert from 'node:assert/strict';

const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const output = path.join(root, '.github/readme/images');
const server = await createServer({
  configFile: false,
  root: path.join(root, 'src/renderer'),
  plugins: [react()],
  resolve: {
    alias: {
      '@shared': path.join(root, 'src/shared'),
      '@renderer': path.join(root, 'src/renderer/src'),
    },
  },
  server: { host: '127.0.0.1', port: 0 },
});
await server.listen();
const base = server.resolvedUrls.local[0];
const browser = await chromium.launch({ channel: 'chrome', headless: true });
await fs.mkdir(output, { recursive: true });
const errors = [];
try {
  async function makePage(theme = 'dark', locked = false) {
    const context = await browser.newContext({
      viewport: { width: 1280, height: 860 },
      reducedMotion: 'reduce',
      colorScheme: theme,
    });
    await context.route('**/*', (route) =>
      new URL(route.request().url()).hostname === '127.0.0.1' ? route.continue() : route.abort(),
    );
    await context.addInitScript(
      ({ theme, locked }) => {
        localStorage.setItem('authapp.language', 'en');
        localStorage.setItem('authapp.theme', theme);
        let settings = { theme, language: 'en', closeToTray: true, startMinimized: false };
        const accounts = ['GitHub', 'Google', 'Linear', 'Figma', 'Notion', 'Dropbox'].map(
          (issuer, index) => ({
            id: `00000000-0000-4000-8000-${String(index + 1).padStart(12, '0')}`,
            issuer,
            accountName: 'alex@example.com',
            algorithm: 'SHA1',
            digits: 6,
            period: 30,
            favorite: index < 2,
            groupId: null,
            sortOrder: index,
            createdAt: 1767225600000,
            updatedAt: 1767225600000,
          }),
        );
        window.authapp = {
          getRuntimeInfo: async () => ({
            appName: 'Authly',
            appVersion: '1.0.0',
            platform: 'win32',
            packaged: false,
          }),
          listAccounts: async () => accounts,
          getTotpCodes: async () =>
            accounts.map((account, index) => ({
              accountId: account.id,
              code: ['123456', '654321', '246810', '135790', '112233', '445566'][index],
              remaining: 22,
              period: 30,
            })),
          getSettings: async () => settings,
          updateSettings: async (changes) => (settings = { ...settings, ...changes }),
          getLockStatus: async () => ({ isPinSet: true, isLocked: locked, autoLockTimeout: 5 }),
          checkUpdate: async () => ({
            hasUpdate: false,
            currentVersion: '1.0.0',
            latestVersion: '1.0.0',
          }),
          getUpdateState: async () => ({ phase: 'idle', currentVersion: '1.0.0' }),
          onUpdateState: () => () => {},
          copyTotp: async () => {},
          lockApp: async () => {},
          recordActivity: async () => {},
          onLockStatus: () => () => {},
        };
      },
      { theme, locked },
    );
    const page = await context.newPage();
    page.on('pageerror', (error) => errors.push(error.message));
    await page.clock.setFixedTime(new Date('2026-01-01T12:00:08Z'));
    await page.goto(base);
    await page
      .getByRole('heading', { name: 'GitHub', exact: true })
      .waitFor({ timeout: 8000 })
      .catch(async (error) => {
        console.error('Renderer errors:', errors);
        console.error((await page.locator('body').innerText()).slice(0, 2500));
        await page.screenshot({ path: path.join(output, 'debug.png') });
        throw error;
      });
    return page;
  }
  async function shot(page, name) {
    await page.evaluate(async () => {
      await document.fonts.ready;
      for (const animation of document.getAnimations()) {
        if (Number.isFinite(animation.effect.getTiming().iterations)) animation.finish();
        else animation.pause();
      }
    });
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth));
    await page.screenshot({ path: path.join(output, name), fullPage: true });
  }
  const dark = await makePage();
  await shot(dark, 'accounts-dark.png');
  await dark.goto(base + '#/add');
  await dark.locator('input').first().waitFor();
  await shot(dark, 'add-account.png');
  await dark.goto(base + '#/settings');
  await dark.getByRole('heading').first().waitFor();
  await shot(dark, 'settings.png');
  const light = await makePage('light');
  await shot(light, 'accounts-light.png');
  const lock = await makePage('dark', true);
  await lock.locator('.lock-screen-overlay, .lock-overlay').first().waitFor();
  await shot(lock, 'pin-lock.png');
  assert.deepEqual(errors, []);
  const data = async (name) =>
    'data:image/png;base64,' + (await fs.readFile(path.join(output, name))).toString('base64');
  const icon =
    'data:image/png;base64,' +
    (await fs.readFile(path.join(root, 'resources/icon.png'))).toString('base64');
  const cover = await browser.newPage({ viewport: { width: 1600, height: 1100 } });
  await cover.setContent(`<!doctype html><meta charset="utf-8"><style>
  *{box-sizing:border-box}body{margin:0;background:#090a0c;color:#f6f6f6;font-family:'Segoe UI',sans-serif}
  .brand{position:absolute;top:58px;left:76px;display:flex;align-items:center;gap:20px;font-size:38px;font-weight:600;letter-spacing:-1px}.brand img{width:58px;height:58px;border-radius:14px}
  .platforms{position:absolute;top:78px;right:76px;font-size:16px;color:#a4a6af}
  h1{position:absolute;left:76px;top:150px;margin:0;font-size:76px;letter-spacing:-3px;line-height:1.1;font-weight:600}h1 span{color:#9a9ca5}
  .description{position:absolute;left:80px;top:253px;color:#b4b6bf;font-size:22px}
  .desktop{position:absolute;left:76px;top:348px;width:1140px;height:640px;object-fit:cover;object-position:top;border-radius:18px;box-shadow:0 24px 70px #0009}
  .lock{position:absolute;right:76px;top:510px;width:310px;height:420px;object-fit:cover;object-position:center;border-radius:18px;box-shadow:0 24px 60px #000b;outline:5px solid #26282d}
  .footer{position:absolute;left:76px;bottom:35px;color:#9b9da7;font-size:14px;letter-spacing:2px}
  </style><div class="brand"><img src="${icon}">Authly</div><div class="platforms">WINDOWS + LINUX</div><h1>Your codes. <span>Your device.</span></h1><div class="description">An offline desktop authenticator, built around you.</div><img class="desktop" src="${await data('accounts-dark.png')}"><img class="lock" src="${await data('pin-lock.png')}"><div class="footer">LOCAL TOTP · OS-ENCRYPTED SECRETS · ENCRYPTED BACKUPS</div>`);
  await cover.evaluate(() => Promise.all([...document.images].map((image) => image.decode())));
  await cover.screenshot({ path: path.join(output, 'cover.png') });
  console.log(
    'PASS: real renderer captured with synthetic accounts/codes; no vault access or renderer exceptions.',
  );
} finally {
  await browser.close();
  await server.close();
}
