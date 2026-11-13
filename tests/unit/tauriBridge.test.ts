import { beforeEach, describe, expect, it, vi } from 'vitest';
import { LOCK_STATUS_EVENT, UPDATE_STATE_EVENT } from '../../src/shared/ipc/channels';
import { createTauriAuthappApi } from '../../src/renderer/src/bridge/tauriBridge';

const { invokeMock, listenMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  listenMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: listenMock,
}));

describe('Tauri window.authapp bridge', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    listenMock.mockReset();
  });

  it('validates and forwards runtime, account, totp, backup, settings, lock and update commands', async () => {
    const api = createTauriAuthappApi();
    const accountId = 'a4ff59c6-2df2-4136-86ba-78bd57a617d8';
    const sampleAccount = {
      id: accountId,
      issuer: 'GitHub',
      accountName: 'arda@example.com',
      algorithm: 'SHA1',
      digits: 6,
      period: 30,
      favorite: false,
      groupId: null,
      sortOrder: 0,
      createdAt: 1,
      updatedAt: 1,
    };

    invokeMock.mockResolvedValueOnce({
      appName: 'Authly',
      appVersion: '0.3.12',
      platform: 'win32',
      packaged: true,
    });
    await expect(api.getRuntimeInfo()).resolves.toMatchObject({
      appName: 'Authly',
      appVersion: '0.3.12',
    });
    expect(invokeMock).toHaveBeenLastCalledWith('get_runtime_info', { request: {} });

    invokeMock.mockResolvedValueOnce([sampleAccount]);
    await expect(api.listAccounts()).resolves.toEqual([sampleAccount]);
    expect(invokeMock).toHaveBeenLastCalledWith('list_accounts', { request: {} });

    invokeMock.mockResolvedValueOnce(sampleAccount);
    await expect(
      api.createAccount({
        issuer: 'GitHub',
        accountName: 'arda@example.com',
        secret: 'jbsw-y3dp ehpk3pxp',
        algorithm: 'SHA1',
        digits: 6,
        period: 30,
        favorite: false,
        groupId: null,
      }),
    ).resolves.toEqual(sampleAccount);
    expect(invokeMock).toHaveBeenLastCalledWith('create_account', {
      request: {
        issuer: 'GitHub',
        accountName: 'arda@example.com',
        secret: 'JBSWY3DPEHPK3PXP',
        algorithm: 'SHA1',
        digits: 6,
        period: 30,
        favorite: false,
        groupId: null,
      },
    });

    invokeMock.mockResolvedValueOnce([{ accountId, code: '123456', remaining: 15, period: 30 }]);
    await expect(api.getTotpCodes()).resolves.toEqual([
      { accountId, code: '123456', remaining: 15, period: 30 },
    ]);
    expect(invokeMock).toHaveBeenLastCalledWith('get_totp_codes', { request: {} });

    invokeMock.mockResolvedValueOnce({
      issuer: 'GitHub',
      accountName: 'arda@example.com',
      secret: 'JBSWY3DPEHPK3PXP',
      algorithm: 'SHA1',
      digits: 6,
      period: 30,
      favorite: false,
      groupId: null,
    });
    await expect(
      api.parseOtpAuthUri('otpauth://totp/GitHub:arda@example.com?secret=JBSWY3DPEHPK3PXP'),
    ).resolves.toMatchObject({ issuer: 'GitHub' });

    invokeMock.mockResolvedValueOnce(undefined);
    await api.copyTotp(accountId);
    expect(invokeMock).toHaveBeenLastCalledWith('copy_totp', { request: { accountId } });

    invokeMock.mockResolvedValueOnce({
      success: true,
      filePath: 'C:/backup.authapp',
      exportedCount: 1,
    });
    await expect(api.exportBackup({ password: 'strong-backup-password' })).resolves.toMatchObject({
      success: true,
      exportedCount: 1,
    });

    invokeMock.mockResolvedValueOnce({
      success: true,
      importedCount: 1,
      skippedCount: 0,
      ungroupedCount: 0,
    });
    await expect(api.importBackup({ password: 'strong-backup-password' })).resolves.toMatchObject({
      success: true,
      importedCount: 1,
    });

    invokeMock.mockResolvedValueOnce({
      theme: 'system',
      language: 'tr',
      closeToTray: true,
      startMinimized: false,
    });
    await expect(api.getSettings()).resolves.toMatchObject({ language: 'tr' });

    invokeMock.mockResolvedValueOnce({
      isPinSet: true,
      isLocked: false,
      autoLockTimeout: 5,
    });
    await expect(api.getLockStatus()).resolves.toMatchObject({ isPinSet: true, isLocked: false });

    invokeMock.mockResolvedValueOnce(true);
    await expect(api.verifyPin({ pin: '1234' })).resolves.toBe(true);

    invokeMock.mockResolvedValueOnce({
      hasUpdate: false,
      currentVersion: '0.3.12',
      latestVersion: '0.3.12',
    });
    await expect(api.checkUpdate()).resolves.toMatchObject({ hasUpdate: false });
  });

  it('normalizes string errors thrown by Tauri invoke into Error instances', async () => {
    const api = createTauriAuthappApi();
    invokeMock.mockRejectedValueOnce('PIN hatalı.');
    await expect(api.verifyPin({ pin: '0000' })).rejects.toThrow('PIN hatalı.');
  });

  it('rejects invalid input before invoking Rust commands', async () => {
    const api = createTauriAuthappApi();
    await expect(
      api.createAccount({
        issuer: 'GitHub',
        accountName: 'arda@example.com',
        secret: 'INVALID!',
        algorithm: 'SHA1',
        digits: 6,
        period: 30,
        favorite: false,
        groupId: null,
      }),
    ).rejects.toThrow();
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('subscribes and safely unsubscribes Tauri events even before listen resolves', async () => {
    const api = createTauriAuthappApi();
    const unlisten = vi.fn();
    let resolveListen!: (fn: () => void) => void;
    listenMock.mockReturnValue(
      new Promise<() => void>((resolve) => {
        resolveListen = resolve;
      }),
    );

    const stop = api.onLockStatus(() => {});
    stop();
    resolveListen(unlisten);
    await Promise.resolve();
    expect(unlisten).toHaveBeenCalledTimes(1);

    let capturedHandler: ((event: { payload: unknown }) => void) | undefined;
    listenMock.mockImplementationOnce(async (_eventName, handler) => {
      capturedHandler = handler;
      return unlisten;
    });
    const received: unknown[] = [];
    const stopUpdate = api.onUpdateState((state) => received.push(state));
    await Promise.resolve();
    capturedHandler?.({ payload: { phase: 'idle', currentVersion: '0.3.12' } });
    expect(listenMock).toHaveBeenCalledWith(LOCK_STATUS_EVENT, expect.any(Function));
    expect(listenMock).toHaveBeenCalledWith(UPDATE_STATE_EVENT, expect.any(Function));
    expect(received).toEqual([{ phase: 'idle', currentVersion: '0.3.12' }]);
    stopUpdate();
  });

  it('falls back cleanly to browser preview storage when Tauri internals are absent', async () => {
    window.localStorage.clear();
    delete (window as unknown as Record<string, unknown>)['__TAURI_INTERNALS__'];
    invokeMock.mockRejectedValue(
      new TypeError("Cannot read properties of undefined (reading 'invoke')"),
    );
    listenMock.mockRejectedValue(
      new TypeError("Cannot read properties of undefined (reading 'transformCallback')"),
    );

    const api = createTauriAuthappApi();

    const lockStatus = await api.getLockStatus();
    expect(lockStatus).toEqual({
      isPinSet: false,
      isLocked: false,
      autoLockTimeout: 5,
    });

    const settings = await api.getSettings();
    expect(settings).toEqual({
      theme: 'system',
      language: 'tr',
      closeToTray: true,
      startMinimized: false,
    });

    const created = await api.createAccount({
      issuer: 'GitHub',
      accountName: 'arda@example.com',
      secret: 'JBSWY3DPEHPK3PXP',
      algorithm: 'SHA1',
      digits: 6,
      period: 30,
      favorite: false,
      groupId: null,
    });
    expect(created).toMatchObject({
      issuer: 'GitHub',
      accountName: 'arda@example.com',
      algorithm: 'SHA1',
      digits: 6,
      period: 30,
    });

    const codes = await api.getTotpCodes();
    expect(codes).toHaveLength(1);
    expect(codes[0]?.accountId).toBe(created.id);
    expect(codes[0]?.code).toMatch(/^\d{6}$/);
    expect(codes[0]?.period).toBe(30);
    window.localStorage.clear();
  });
});
