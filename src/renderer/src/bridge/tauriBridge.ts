import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { LOCK_STATUS_EVENT, UPDATE_STATE_EVENT } from '@shared/ipc/channels';
import {
  accountDtoSchema,
  createAccountRequestSchema,
  deleteAccountRequestSchema,
  importAccountsRequestSchema,
  listAccountsRequestSchema,
  updateAccountRequestSchema,
  type CreateAccountRequest,
  type DeleteAccountRequest,
  type ImportAccountsRequest,
  type UpdateAccountRequest,
} from '@shared/schemas/account';
import {
  exportBackupRequestSchema,
  exportBackupResultSchema,
  importBackupRequestSchema,
  importBackupResultSchema,
  type ExportBackupRequest,
  type ImportBackupRequest,
} from '@shared/schemas/backup';
import {
  lockActivityRequestSchema,
  lockStatusSchema,
  removePinRequestSchema,
  setPinRequestSchema,
  verifyPinRequestSchema,
  type RemovePinRequest,
  type SetPinRequest,
  type VerifyPinRequest,
} from '@shared/schemas/lock';
import { runtimeInfoRequestSchema, runtimeInfoSchema } from '@shared/schemas/runtime';
import {
  appSettingsSchema,
  getSettingsRequestSchema,
  updateSettingsRequestSchema,
  type UpdateSettingsRequest,
} from '@shared/schemas/settings';
import {
  copyTotpRequestSchema,
  getTotpCodesRequestSchema,
  parsedOtpAuthSchema,
  parseOtpAuthUriRequestSchema,
  totpCodeSchema,
} from '@shared/schemas/totp';
import {
  checkUpdateRequestSchema,
  getUpdateStateRequestSchema,
  installUpdateRequestSchema,
  updateInfoSchema,
  updateStateSchema,
} from '@shared/schemas/update';
import type { AuthappApi } from '@shared/types/electron-api';

const PREVIEW_VERSION = '0.3.12';
const BASE32_ALPHABET = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567';
const PREVIEW_STORAGE_KEYS = {
  accounts: 'authapp.preview.accounts',
  settings: 'authapp.preview.settings',
  pin: 'authapp.preview.pin',
  locked: 'authapp.preview.locked',
  autoLockTimeout: 'authapp.preview.autoLockTimeout',
} as const;

type TotpAlgorithm = 'SHA1' | 'SHA256' | 'SHA512';

interface StoredPreviewAccount {
  id: string;
  issuer: string;
  accountName: string;
  secret: string;
  algorithm: TotpAlgorithm;
  digits: number;
  period: number;
  favorite: boolean;
  groupId: string | null;
  sortOrder: number;
  createdAt: number;
  updatedAt: number;
}

const previewListeners = new Map<string, Set<(payload: unknown) => void>>();

function isPreviewMode(): boolean {
  return typeof window !== 'undefined' && !('__TAURI_INTERNALS__' in window);
}

function emitPreviewEvent(eventName: string, payload: unknown): void {
  const listeners = previewListeners.get(eventName);
  if (!listeners) return;
  for (const listener of listeners) {
    listener(payload);
  }
}

function readPreviewJson<T>(key: string, fallback: T): T {
  try {
    const raw = window.localStorage.getItem(key);
    if (!raw) return fallback;
    return JSON.parse(raw) as T;
  } catch {
    return fallback;
  }
}

function writePreviewJson(key: string, value: unknown): void {
  window.localStorage.setItem(key, JSON.stringify(value));
}

function readPreviewAccounts(): StoredPreviewAccount[] {
  const parsed = readPreviewJson<unknown>(PREVIEW_STORAGE_KEYS.accounts, []);
  return Array.isArray(parsed) ? (parsed as StoredPreviewAccount[]) : [];
}

function toAccountDto(account: StoredPreviewAccount) {
  return {
    id: account.id,
    issuer: account.issuer,
    accountName: account.accountName,
    algorithm: account.algorithm,
    digits: account.digits,
    period: account.period,
    favorite: account.favorite,
    groupId: account.groupId,
    sortOrder: account.sortOrder,
    createdAt: account.createdAt,
    updatedAt: account.updatedAt,
  };
}

function createPreviewUuid(): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return crypto.randomUUID();
  }
  const bytes = new Uint8Array(16);
  if (typeof crypto !== 'undefined' && typeof crypto.getRandomValues === 'function') {
    crypto.getRandomValues(bytes);
  }
  bytes[6] = (bytes[6]! & 0x0f) | 0x40;
  bytes[8] = (bytes[8]! & 0x3f) | 0x80;
  const hex = Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

function readPreviewLockStatus() {
  const pin = window.localStorage.getItem(PREVIEW_STORAGE_KEYS.pin);
  const isPinSet = Boolean(pin);
  const isLocked = isPinSet && window.localStorage.getItem(PREVIEW_STORAGE_KEYS.locked) === 'true';
  const rawValue = window.localStorage.getItem(PREVIEW_STORAGE_KEYS.autoLockTimeout);
  const rawTimeout = rawValue === null ? Number.NaN : Number(rawValue);
  const autoLockTimeout = Number.isInteger(rawTimeout) && rawTimeout >= 0 ? rawTimeout : 5;
  return { isPinSet, isLocked, autoLockTimeout };
}

function assertPreviewUnlocked(): void {
  if (readPreviewLockStatus().isLocked) {
    throw new Error('Uygulama kilitli.');
  }
}

function readPreviewSettings() {
  const defaults = {
    theme: 'system' as const,
    language: 'tr' as const,
    closeToTray: true,
    startMinimized: false,
  };
  const stored = readPreviewJson<Record<string, unknown>>(PREVIEW_STORAGE_KEYS.settings, {});
  return appSettingsSchema.parse({ ...defaults, ...stored });
}

function decodeBase32(secret: string): Uint8Array<ArrayBuffer> {
  const clean = secret.replace(/[\s=-]/g, '').toUpperCase();
  const output: number[] = [];
  let buffer = 0;
  let bits = 0;

  for (const ch of clean) {
    const val = BASE32_ALPHABET.indexOf(ch);
    if (val < 0) continue;
    buffer = (buffer << 5) | val;
    bits += 5;
    if (bits >= 8) {
      bits -= 8;
      output.push((buffer >>> bits) & 0xff);
    }
  }

  const bytes = new Uint8Array(new ArrayBuffer(output.length));
  bytes.set(output);
  return bytes;
}

async function generatePreviewTotp(
  secret: string,
  algorithm: TotpAlgorithm,
  digits: number,
  period: number,
): Promise<{ code: string; remaining: number; period: number }> {
  const nowMs = Date.now();
  const counter = Math.floor(nowMs / 1000 / period);
  const counterBuf = new ArrayBuffer(8);
  const view = new DataView(counterBuf);
  view.setUint32(0, Math.floor(counter / 0x100000000), false);
  view.setUint32(4, counter >>> 0, false);

  const hashName =
    algorithm === 'SHA256' ? 'SHA-256' : algorithm === 'SHA512' ? 'SHA-512' : 'SHA-1';
  const keyBytes = decodeBase32(secret);
  const key = await crypto.subtle.importKey(
    'raw',
    keyBytes,
    { name: 'HMAC', hash: hashName },
    false,
    ['sign'],
  );
  const signature = new Uint8Array(await crypto.subtle.sign('HMAC', key, counterBuf));
  const offset = signature[signature.length - 1]! & 0x0f;
  const binCode =
    ((signature[offset]! & 0x7f) << 24) |
    ((signature[offset + 1]! & 0xff) << 16) |
    ((signature[offset + 2]! & 0xff) << 8) |
    (signature[offset + 3]! & 0xff);
  const otp = binCode % 10 ** digits;
  const code = String(otp).padStart(digits, '0');

  const periodMs = period * 1000;
  const remMs = periodMs - (nowMs % periodMs);
  const remaining = Math.max(1, Math.ceil(remMs / 1000));
  return { code, remaining, period };
}

function parsePreviewOtpAuthUri(uri: string) {
  let parsed: URL;
  try {
    parsed = new URL(uri);
  } catch (err) {
    throw new Error('otpauth bağlantısı okunamadı.', { cause: err });
  }
  const normalizedPath = parsed.pathname.replace(/^\/+/, '');
  const hostIsTotp = parsed.hostname === 'totp' || normalizedPath.startsWith('totp/');
  if (parsed.protocol !== 'otpauth:' || !hostIsTotp) {
    throw new Error('Only TOTP URIs are supported.');
  }

  const rawLabel =
    parsed.hostname === 'totp' ? normalizedPath : normalizedPath.slice('totp/'.length);
  const decodedLabel = decodeURIComponent(rawLabel);
  const colonIdx = decodedLabel.indexOf(':');
  const labelIssuer = colonIdx >= 0 ? decodedLabel.slice(0, colonIdx).trim() : '';
  const rawAccountName = (colonIdx >= 0 ? decodedLabel.slice(colonIdx + 1) : decodedLabel).trim();

  const secret = parsed.searchParams.get('secret');
  if (!secret) {
    throw new Error('otpauth bağlantısında gizli anahtar eksik.');
  }
  const queryIssuer = parsed.searchParams.get('issuer')?.trim() ?? '';
  const rawAlg = parsed.searchParams.get('algorithm')?.toUpperCase() ?? 'SHA1';
  const digitsParam = parsed.searchParams.get('digits');
  const periodParam = parsed.searchParams.get('period');
  const issuer = queryIssuer || labelIssuer || 'Authenticator';
  const accountName = rawAccountName || issuer;

  return createAccountRequestSchema.parse({
    issuer,
    accountName,
    secret,
    algorithm: rawAlg,
    digits: digitsParam ? Number(digitsParam) : 6,
    period: periodParam ? Number(periodParam) : 30,
    favorite: false,
    groupId: null,
  });
}

async function invokeBrowserPreview<T>(command: string, args: Record<string, unknown>): Promise<T> {
  const request = (args['request'] ?? {}) as Record<string, unknown>;

  switch (command) {
    case 'get_runtime_info':
      return {
        appName: 'Authly',
        appVersion: PREVIEW_VERSION,
        platform: 'win32',
        packaged: false,
      } as T;

    case 'get_settings':
      return readPreviewSettings() as T;

    case 'update_settings': {
      const next = appSettingsSchema.parse({ ...readPreviewSettings(), ...request });
      writePreviewJson(PREVIEW_STORAGE_KEYS.settings, next);
      return next as T;
    }

    case 'get_lock_status':
      return readPreviewLockStatus() as T;

    case 'set_pin': {
      const existingPin = window.localStorage.getItem(PREVIEW_STORAGE_KEYS.pin);
      const parsed = setPinRequestSchema.parse(request);
      if (existingPin && parsed.currentPin !== existingPin) {
        throw new Error('Mevcut PIN hatalı.');
      }
      window.localStorage.setItem(PREVIEW_STORAGE_KEYS.pin, parsed.pin);
      window.localStorage.setItem(PREVIEW_STORAGE_KEYS.locked, 'false');
      emitPreviewEvent(LOCK_STATUS_EVENT, readPreviewLockStatus());
      return undefined as T;
    }

    case 'verify_pin': {
      const existingPin = window.localStorage.getItem(PREVIEW_STORAGE_KEYS.pin);
      const parsed = verifyPinRequestSchema.parse(request);
      if (!existingPin || parsed.pin !== existingPin) {
        throw new Error('PIN hatalı.');
      }
      window.localStorage.setItem(PREVIEW_STORAGE_KEYS.locked, 'false');
      emitPreviewEvent(LOCK_STATUS_EVENT, readPreviewLockStatus());
      return true as T;
    }

    case 'remove_pin': {
      const existingPin = window.localStorage.getItem(PREVIEW_STORAGE_KEYS.pin);
      const parsed = removePinRequestSchema.parse(request);
      if (existingPin && parsed.currentPin !== existingPin) {
        throw new Error('Mevcut PIN hatalı.');
      }
      window.localStorage.removeItem(PREVIEW_STORAGE_KEYS.pin);
      window.localStorage.removeItem(PREVIEW_STORAGE_KEYS.locked);
      emitPreviewEvent(LOCK_STATUS_EVENT, readPreviewLockStatus());
      return undefined as T;
    }

    case 'lock_app': {
      if (window.localStorage.getItem(PREVIEW_STORAGE_KEYS.pin)) {
        window.localStorage.setItem(PREVIEW_STORAGE_KEYS.locked, 'true');
        emitPreviewEvent(LOCK_STATUS_EVENT, readPreviewLockStatus());
      }
      return undefined as T;
    }

    case 'record_activity':
      return undefined as T;

    case 'list_accounts': {
      assertPreviewUnlocked();
      return readPreviewAccounts().map(toAccountDto) as T;
    }

    case 'create_account': {
      assertPreviewUnlocked();
      const input = createAccountRequestSchema.parse(request);
      const accounts = readPreviewAccounts();
      const now = Math.max(1, Date.now());
      const nextSortOrder = accounts.reduce((max, item) => Math.max(max, item.sortOrder), -1) + 1;
      const created: StoredPreviewAccount = {
        id: createPreviewUuid(),
        issuer: input.issuer,
        accountName: input.accountName,
        secret: input.secret,
        algorithm: input.algorithm,
        digits: input.digits,
        period: input.period,
        favorite: input.favorite,
        groupId: input.groupId,
        sortOrder: nextSortOrder,
        createdAt: now,
        updatedAt: now,
      };
      accounts.push(created);
      writePreviewJson(PREVIEW_STORAGE_KEYS.accounts, accounts);
      return toAccountDto(created) as T;
    }

    case 'import_accounts': {
      assertPreviewUnlocked();
      const input = importAccountsRequestSchema.parse(request);
      const accounts = readPreviewAccounts();
      const now = Math.max(1, Date.now());
      let nextSortOrder = accounts.reduce((max, item) => Math.max(max, item.sortOrder), -1) + 1;
      const added: StoredPreviewAccount[] = input.accounts.map((item) => ({
        id: createPreviewUuid(),
        issuer: item.issuer,
        accountName: item.accountName,
        secret: item.secret,
        algorithm: item.algorithm,
        digits: item.digits,
        period: item.period,
        favorite: item.favorite,
        groupId: item.groupId,
        sortOrder: nextSortOrder++,
        createdAt: now,
        updatedAt: now,
      }));
      accounts.push(...added);
      writePreviewJson(PREVIEW_STORAGE_KEYS.accounts, accounts);
      return added.map(toAccountDto) as T;
    }

    case 'update_account': {
      assertPreviewUnlocked();
      const input = updateAccountRequestSchema.parse(request);
      const accounts = readPreviewAccounts();
      const idx = accounts.findIndex((item) => item.id === input.id);
      if (idx < 0) throw new Error('Hesap bulunamadı.');
      const current = accounts[idx]!;
      const updated: StoredPreviewAccount = {
        ...current,
        issuer: input.issuer,
        accountName: input.accountName,
        secret: input.secret ?? current.secret,
        algorithm: input.algorithm,
        digits: input.digits,
        period: input.period,
        favorite: input.favorite,
        groupId: input.groupId,
        updatedAt: Math.max(1, Date.now()),
      };
      accounts[idx] = updated;
      writePreviewJson(PREVIEW_STORAGE_KEYS.accounts, accounts);
      return toAccountDto(updated) as T;
    }

    case 'delete_account': {
      assertPreviewUnlocked();
      const input = deleteAccountRequestSchema.parse(request);
      const filtered = readPreviewAccounts().filter((item) => item.id !== input.id);
      writePreviewJson(PREVIEW_STORAGE_KEYS.accounts, filtered);
      return undefined as T;
    }

    case 'get_totp_codes': {
      assertPreviewUnlocked();
      const accounts = readPreviewAccounts();
      const codes = await Promise.all(
        accounts.map(async (acc) => {
          const totp = await generatePreviewTotp(acc.secret, acc.algorithm, acc.digits, acc.period);
          return { accountId: acc.id, ...totp };
        }),
      );
      return codes as T;
    }

    case 'parse_otp_auth_uri': {
      const uri = String(request['uri'] ?? '');
      return parsePreviewOtpAuthUri(uri) as T;
    }

    case 'copy_totp': {
      assertPreviewUnlocked();
      const input = copyTotpRequestSchema.parse(request);
      const account = readPreviewAccounts().find((item) => item.id === input.accountId);
      if (!account) throw new Error('Hesap bulunamadı.');
      const totp = await generatePreviewTotp(
        account.secret,
        account.algorithm,
        account.digits,
        account.period,
      );
      if (typeof navigator !== 'undefined' && navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(totp.code).catch(() => {});
      }
      return undefined as T;
    }

    case 'export_backup':
      assertPreviewUnlocked();
      return { success: false, filePath: null, exportedCount: 0 } as T;

    case 'import_backup':
      assertPreviewUnlocked();
      return { success: false, importedCount: 0, skippedCount: 0, ungroupedCount: 0 } as T;

    case 'check_update':
      return {
        hasUpdate: false,
        currentVersion: PREVIEW_VERSION,
        latestVersion: PREVIEW_VERSION,
      } as T;

    case 'get_update_state':
      return {
        phase: 'idle',
        currentVersion: PREVIEW_VERSION,
      } as T;

    case 'install_update':
      return undefined as T;

    default:
      throw new Error(`Desteklenmeyen komut: ${command}`);
  }
}

async function invokeTauri<T>(command: string, args: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    if (error instanceof TypeError && isPreviewMode()) {
      return await invokeBrowserPreview<T>(command, args);
    }
    if (error instanceof Error) throw error;
    throw new Error(typeof error === 'string' ? error : 'İşlem başarısız.', { cause: error });
  }
}

function subscribeTauriEvent<T>(
  eventName: string,
  parse: (payload: unknown) => T,
  listener: (payload: T) => void,
): () => void {
  let active = true;
  let unlisten: UnlistenFn | null = null;

  const previewHandler = (payload: unknown): void => {
    if (!active) return;
    listener(parse(payload));
  };

  let set = previewListeners.get(eventName);
  if (!set) {
    set = new Set();
    previewListeners.set(eventName, set);
  }
  set.add(previewHandler);

  void listen(eventName, (event) => {
    if (!active) return;
    listener(parse(event.payload));
  })
    .then((fn) => {
      if (active) {
        unlisten = fn;
      } else {
        fn();
      }
    })
    .catch(() => {});

  return () => {
    active = false;
    previewListeners.get(eventName)?.delete(previewHandler);
    if (unlisten) {
      unlisten();
      unlisten = null;
    }
  };
}

export function createTauriAuthappApi(): AuthappApi {
  return Object.freeze({
    getRuntimeInfo: async () =>
      runtimeInfoSchema.parse(
        await invokeTauri('get_runtime_info', {
          request: runtimeInfoRequestSchema.parse({}),
        }),
      ),
    listAccounts: async () =>
      accountDtoSchema.array().parse(
        await invokeTauri('list_accounts', {
          request: listAccountsRequestSchema.parse({}),
        }),
      ),
    createAccount: async (request: CreateAccountRequest) =>
      accountDtoSchema.parse(
        await invokeTauri('create_account', {
          request: createAccountRequestSchema.parse(request),
        }),
      ),
    importAccounts: async (request: ImportAccountsRequest) =>
      accountDtoSchema.array().parse(
        await invokeTauri('import_accounts', {
          request: importAccountsRequestSchema.parse(request),
        }),
      ),
    updateAccount: async (request: UpdateAccountRequest) =>
      accountDtoSchema.parse(
        await invokeTauri('update_account', {
          request: updateAccountRequestSchema.parse(request),
        }),
      ),
    deleteAccount: async (request: DeleteAccountRequest) => {
      await invokeTauri('delete_account', {
        request: deleteAccountRequestSchema.parse(request),
      });
    },
    getTotpCodes: async () =>
      totpCodeSchema.array().parse(
        await invokeTauri('get_totp_codes', {
          request: getTotpCodesRequestSchema.parse({}),
        }),
      ),
    parseOtpAuthUri: async (uri: string) =>
      parsedOtpAuthSchema.parse(
        await invokeTauri('parse_otp_auth_uri', {
          request: parseOtpAuthUriRequestSchema.parse({ uri }),
        }),
      ),
    copyTotp: async (accountId: string) => {
      await invokeTauri('copy_totp', {
        request: copyTotpRequestSchema.parse({ accountId }),
      });
    },
    exportBackup: async (request: ExportBackupRequest) =>
      exportBackupResultSchema.parse(
        await invokeTauri('export_backup', {
          request: exportBackupRequestSchema.parse(request),
        }),
      ),
    importBackup: async (request: ImportBackupRequest) =>
      importBackupResultSchema.parse(
        await invokeTauri('import_backup', {
          request: importBackupRequestSchema.parse(request),
        }),
      ),
    getSettings: async () =>
      appSettingsSchema.parse(
        await invokeTauri('get_settings', {
          request: getSettingsRequestSchema.parse({}),
        }),
      ),
    updateSettings: async (request: UpdateSettingsRequest) =>
      appSettingsSchema.parse(
        await invokeTauri('update_settings', {
          request: updateSettingsRequestSchema.parse(request),
        }),
      ),
    getLockStatus: async () =>
      lockStatusSchema.parse(await invokeTauri('get_lock_status', { request: {} })),
    verifyPin: async (request: VerifyPinRequest) => {
      const result = await invokeTauri<boolean>('verify_pin', {
        request: verifyPinRequestSchema.parse(request),
      });
      return Boolean(result);
    },
    setPin: async (request: SetPinRequest) => {
      await invokeTauri('set_pin', {
        request: setPinRequestSchema.parse(request),
      });
    },
    removePin: async (request: RemovePinRequest) => {
      await invokeTauri('remove_pin', {
        request: removePinRequestSchema.parse(request),
      });
    },
    lockApp: async () => {
      await invokeTauri('lock_app', { request: {} });
    },
    recordActivity: async () => {
      await invokeTauri('record_activity', {
        request: lockActivityRequestSchema.parse({}),
      });
    },
    onLockStatus: (listener: Parameters<AuthappApi['onLockStatus']>[0]) =>
      subscribeTauriEvent(
        LOCK_STATUS_EVENT,
        (payload) => lockStatusSchema.parse(payload),
        listener,
      ),
    checkUpdate: async () =>
      updateInfoSchema.parse(
        await invokeTauri('check_update', {
          request: checkUpdateRequestSchema.parse({}),
        }),
      ),
    getUpdateState: async () =>
      updateStateSchema.parse(
        await invokeTauri('get_update_state', {
          request: getUpdateStateRequestSchema.parse({}),
        }),
      ),
    installUpdate: async () => {
      await invokeTauri('install_update', {
        request: installUpdateRequestSchema.parse({}),
      });
    },
    onUpdateState: (listener: Parameters<AuthappApi['onUpdateState']>[0]) =>
      subscribeTauriEvent(
        UPDATE_STATE_EVENT,
        (payload) => updateStateSchema.parse(payload),
        listener,
      ),
  });
}

export function installTauriBridge(): void {
  if (typeof window !== 'undefined' && !('authapp' in window)) {
    Object.defineProperty(window, 'authapp', {
      value: createTauriAuthappApi(),
      configurable: true,
      writable: false,
    });
  }
}
