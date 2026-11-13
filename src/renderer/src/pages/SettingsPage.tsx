import {
  Download,
  Globe,
  HardDrive,
  KeyRound,
  Lock,
  Monitor,
  Moon,
  Palette,
  RefreshCw,
  ShieldCheck,
  Sun,
  Trash2,
  Upload,
  X,
} from 'lucide-react';
import { useEffect, useState } from 'react';
import { PageHeading } from '../components/PageHeading';
import { useTheme } from '../hooks/useTheme';
import { useLanguage } from '../hooks/useLanguage';
import type { ThemePreference } from '../app/themeContext';
import type { Language } from '../utils/translations';
import type { LockStatus } from '@shared/schemas/lock';
import type { AppSettings } from '@shared/schemas/settings';
import type { UpdateInfo } from '@shared/schemas/update';
import { useUpdateState } from '../hooks/useUpdateState';

const themes: ReadonlyArray<{
  value: ThemePreference;
  labelKey: 'themeSystem' | 'themeLight' | 'themeDark';
  icon: typeof Monitor;
}> = [
  { value: 'system', labelKey: 'themeSystem', icon: Monitor },
  { value: 'light', labelKey: 'themeLight', icon: Sun },
  { value: 'dark', labelKey: 'themeDark', icon: Moon },
];

const languages: ReadonlyArray<{
  value: Language;
  labelKey: 'langTr' | 'langEn';
}> = [
  { value: 'tr', labelKey: 'langTr' },
  { value: 'en', labelKey: 'langEn' },
];

export function SettingsPage(): React.JSX.Element {
  const { preference, setPreference } = useTheme();
  const { language, setLanguage, t } = useLanguage();
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [lockStatus, setLockStatus] = useState<LockStatus | null>(null);
  const [pinFormMode, setPinFormMode] = useState<'set' | 'change' | 'remove' | null>(null);
  const [currentPin, setCurrentPin] = useState('');
  const [newPin, setNewPin] = useState('');
  const [confirmPin, setConfirmPin] = useState('');
  const [exportPassword, setExportPassword] = useState('');
  const [importPassword, setImportPassword] = useState('');
  const [showExportForm, setShowExportForm] = useState(false);
  const [showImportForm, setShowImportForm] = useState(false);
  const [statusMsg, setStatusMsg] = useState<{ type: 'success' | 'error'; message: string } | null>(
    null,
  );
  const [busy, setBusy] = useState(false);
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
  const [appVersion, setAppVersion] = useState<string | null>(null);
  const [updateCheckFailed, setUpdateCheckFailed] = useState(false);
  const [checkingUpdate, setCheckingUpdate] = useState(false);
  const updateState = useUpdateState();
  const updateAvailable =
    updateInfo?.hasUpdate === true ||
    updateState?.phase === 'available' ||
    updateState?.phase === 'downloading' ||
    updateState?.phase === 'downloaded';
  const downloadingUpdate = updateState?.phase === 'downloading';
  const installingUpdate = updateState?.phase === 'downloaded';
  const updateFailed = updateState?.phase === 'error';
  const updateProgress = Math.round(updateState?.progress ?? 0);

  useEffect(() => {
    void window.authapp
      ?.getSettings?.()
      ?.then(setSettings)
      ?.catch(() => {});
    void window.authapp
      ?.getRuntimeInfo?.()
      ?.then((info) => setAppVersion(info.appVersion))
      ?.catch(() => {});
  }, []);

  useEffect(() => {
    let isMounted = true;
    let receivedEvent = false;
    const unsubscribe = window.authapp?.onLockStatus?.((next) => {
      receivedEvent = true;
      if (isMounted) setLockStatus(next);
    });
    void window.authapp
      ?.getLockStatus?.()
      ?.then((next) => {
        if (isMounted && !receivedEvent) setLockStatus(next);
      })
      ?.catch(() => {});
    return () => {
      isMounted = false;
      unsubscribe?.();
    };
  }, []);

  const toggleCloseToTray = async () => {
    if (!settings) return;
    const next = !settings.closeToTray;
    try {
      const updated = await window.authapp.updateSettings({ closeToTray: next });
      setSettings(updated);
    } catch (err) {
      setStatusMsg({
        type: 'error',
        message: err instanceof Error ? err.message : t('settingsUpdateFailed'),
      });
    }
  };

  const openPinForm = (mode: 'set' | 'change' | 'remove') => {
    setPinFormMode((prev) => (prev === mode ? null : mode));
    setShowExportForm(false);
    setShowImportForm(false);
    setCurrentPin('');
    setNewPin('');
    setConfirmPin('');
    setStatusMsg(null);
  };

  const submitPinUpdate = async (e: React.FormEvent, requireCurrent: boolean) => {
    e.preventDefault();
    if (requireCurrent && !currentPin) {
      setStatusMsg({ type: 'error', message: t('currentPinRequired') });
      return;
    }
    const len = [...newPin].length;
    if (len < 4) {
      setStatusMsg({ type: 'error', message: t('pinTooShort') });
      return;
    }
    if (len > 12) {
      setStatusMsg({ type: 'error', message: t('pinTooLong') });
      return;
    }
    if (newPin !== confirmPin) {
      setStatusMsg({ type: 'error', message: t('pinMismatchError') });
      return;
    }
    setBusy(true);
    setStatusMsg(null);
    try {
      await window.authapp.setPin(requireCurrent ? { pin: newPin, currentPin } : { pin: newPin });
      const refreshed = await window.authapp?.getLockStatus?.()?.catch(() => null);
      setLockStatus((prev) =>
        refreshed?.isPinSet
          ? refreshed
          : {
              isPinSet: true,
              isLocked: false,
              autoLockTimeout: refreshed?.autoLockTimeout ?? prev?.autoLockTimeout ?? 5,
            },
      );
      setPinFormMode(null);
      setCurrentPin('');
      setNewPin('');
      setConfirmPin('');
      setStatusMsg({
        type: 'success',
        message: requireCurrent ? t('pinChangedSuccess') : t('pinSetSuccess'),
      });
    } catch (err) {
      setStatusMsg({
        type: 'error',
        message: err instanceof Error ? err.message : t('pinSetFailed'),
      });
    } finally {
      setBusy(false);
    }
  };

  const handleRemovePin = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!currentPin) {
      setStatusMsg({ type: 'error', message: t('currentPinRequired') });
      return;
    }
    setBusy(true);
    setStatusMsg(null);
    try {
      await window.authapp.removePin({ currentPin });
      const refreshed = await window.authapp?.getLockStatus?.()?.catch(() => null);
      setLockStatus((prev) =>
        refreshed && !refreshed.isPinSet
          ? refreshed
          : {
              isPinSet: false,
              isLocked: false,
              autoLockTimeout: refreshed?.autoLockTimeout ?? prev?.autoLockTimeout ?? 5,
            },
      );
      setPinFormMode(null);
      setCurrentPin('');
      setNewPin('');
      setConfirmPin('');
      setStatusMsg({ type: 'success', message: t('pinRemovedSuccess') });
    } catch (err) {
      setStatusMsg({
        type: 'error',
        message: err instanceof Error ? err.message : t('pinRemoveFailed'),
      });
    } finally {
      setBusy(false);
    }
  };

  const handleLockNow = async () => {
    setBusy(true);
    setStatusMsg(null);
    try {
      await window.authapp.lockApp();
      setLockStatus((prev) => (prev ? { ...prev, isLocked: true } : prev));
    } catch (err) {
      setStatusMsg({
        type: 'error',
        message: err instanceof Error ? err.message : t('pinLockFailed'),
      });
    } finally {
      setBusy(false);
    }
  };

  const handleCheckUpdate = async () => {
    setCheckingUpdate(true);
    setUpdateCheckFailed(false);
    try {
      const info = await window.authapp.checkUpdate();
      setUpdateInfo(info);
    } catch {
      setUpdateCheckFailed(true);
    } finally {
      setCheckingUpdate(false);
    }
  };

  const handleInstallUpdate = async () => {
    setUpdateCheckFailed(false);
    try {
      await window.authapp.installUpdate();
    } catch {
      setUpdateCheckFailed(true);
    }
  };

  const handleExport = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!exportPassword || exportPassword.length < 12) {
      setStatusMsg({ type: 'error', message: t('backupPasswordTooShort') });
      return;
    }
    setBusy(true);
    setStatusMsg(null);
    try {
      const result = await window.authapp.exportBackup({ password: exportPassword });
      if (result.success) {
        setStatusMsg({
          type: 'success',
          message: `${result.exportedCount} ${t('exportSuccess')}`,
        });
        setShowExportForm(false);
        setExportPassword('');
      }
    } catch (err) {
      setStatusMsg({
        type: 'error',
        message: err instanceof Error ? err.message : t('backupExportFailed'),
      });
    } finally {
      setBusy(false);
    }
  };

  const handleImport = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!importPassword) {
      setStatusMsg({ type: 'error', message: t('backupPasswordRequired') });
      return;
    }
    setBusy(true);
    setStatusMsg(null);
    try {
      const result = await window.authapp.importBackup({ password: importPassword });
      if (result.success && result.skippedCount === 0) {
        setStatusMsg({
          type: 'success',
          message: `${result.importedCount} ${t('importSuccess')}${result.ungroupedCount ? ` ${t('legacyGroupsRestored')}` : ''}`,
        });
        setShowImportForm(false);
        setImportPassword('');
      } else if (result.skippedCount > 0) {
        setStatusMsg({ type: 'error', message: t('backupIncomplete') });
      }
    } catch (err) {
      setStatusMsg({
        type: 'error',
        message: err instanceof Error ? err.message : t('backupImportFailed'),
      });
    } finally {
      setBusy(false);
    }
  };

  const showPinUpdateForm =
    (!lockStatus?.isPinSet && pinFormMode === 'set') ||
    (lockStatus?.isPinSet && pinFormMode === 'change');

  return (
    <section className="workspace settings-minimal-wrapper">
      <PageHeading title={t('settingsTitle')} description={t('settingsSubtitle')} />

      {statusMsg ? (
        <div
          className={statusMsg.type === 'success' ? 'success-banner' : 'error-banner'}
          role="status"
        >
          {statusMsg.message}
        </div>
      ) : null}

      <div className="settings-minimal-list">
        <section className="setting-row-minimal">
          <div className="setting-label-group">
            <div className="setting-icon-box">
              <Palette size={20} />
            </div>
            <h2 className="setting-title">{t('appearance')}</h2>
          </div>
          <div className="glass-pill-group" role="group" aria-label={t('appearance')}>
            {themes.map(({ value, labelKey, icon: Icon }) => (
              <button
                type="button"
                key={value}
                className={`glass-pill-btn ${preference === value ? 'is-active' : ''}`}
                aria-pressed={preference === value}
                onClick={() => setPreference(value)}
              >
                <Icon size={14} aria-hidden="true" />
                <span>{t(labelKey)}</span>
              </button>
            ))}
          </div>
        </section>

        <section className="setting-row-minimal">
          <div className="setting-label-group">
            <div className="setting-icon-box">
              <Globe size={20} />
            </div>
            <h2 className="setting-title">{t('language')}</h2>
          </div>
          <div className="glass-pill-group" role="group" aria-label={t('language')}>
            {languages.map(({ value, labelKey }) => (
              <button
                type="button"
                key={value}
                className={`glass-pill-btn ${language === value ? 'is-active' : ''}`}
                aria-pressed={language === value}
                onClick={() => setLanguage(value)}
              >
                <span>{t(labelKey)}</span>
              </button>
            ))}
          </div>
        </section>

        <section className="setting-row-minimal">
          <div className="setting-label-group">
            <div className="setting-icon-box">
              <HardDrive size={20} />
            </div>
            <h2 className="setting-title">{t('trayBehavior')}</h2>
          </div>
          <button
            type="button"
            className={`toggle-switch-btn ${settings?.closeToTray ? 'is-checked' : ''}`}
            onClick={() => void toggleCloseToTray()}
            aria-label={t('trayBehavior')}
          >
            <span className="toggle-switch-dot" />
          </button>
        </section>

        <section className="setting-row-minimal">
          <div className="setting-label-group">
            <div className="setting-icon-box">
              <Lock size={20} />
            </div>
            <div style={{ display: 'flex', flexDirection: 'column' }}>
              <h2 className="setting-title">{t('pinSecurity')}</h2>
              <span
                style={{
                  fontSize: '0.78rem',
                  color: lockStatus?.isPinSet ? '#34d399' : 'var(--color-muted)',
                  fontWeight: 500,
                }}
              >
                {lockStatus?.isPinSet ? t('pinEnabled') : t('pinDisabled')}
              </span>
            </div>
          </div>
          <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', flexWrap: 'wrap' }}>
            {!lockStatus?.isPinSet ? (
              <button
                type="button"
                className="glass-pill-btn secondary-link"
                onClick={() => openPinForm('set')}
              >
                <KeyRound size={14} />
                <span>{t('setPinBtn')}</span>
              </button>
            ) : (
              <>
                <button
                  type="button"
                  className="glass-pill-btn secondary-link"
                  onClick={() => openPinForm('change')}
                >
                  <KeyRound size={14} />
                  <span>{t('changePinBtn')}</span>
                </button>
                <button
                  type="button"
                  className="glass-pill-btn secondary-link"
                  onClick={() => openPinForm('remove')}
                >
                  <Trash2 size={14} />
                  <span>{t('removePinBtn')}</span>
                </button>
                <button
                  type="button"
                  className="glass-pill-btn secondary-link"
                  disabled={busy}
                  onClick={() => void handleLockNow()}
                >
                  <Lock size={14} />
                  <span>{t('lockNowBtn')}</span>
                </button>
              </>
            )}
          </div>
        </section>

        {showPinUpdateForm && (
          <form
            className="account-form-card"
            onSubmit={(e) => void submitPinUpdate(e, pinFormMode === 'change')}
            style={{ marginTop: '1rem' }}
          >
            <div
              className="form-card-header"
              style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
              }}
            >
              <div>
                <h3>{pinFormMode === 'change' ? t('changePinTitle') : t('setPinTitle')}</h3>
                <p>{pinFormMode === 'change' ? t('changePinDesc') : t('setPinDesc')}</p>
              </div>
              <button
                type="button"
                className="icon-button"
                aria-label={t('cancel')}
                onClick={() => setPinFormMode(null)}
              >
                <X size={16} />
              </button>
            </div>
            <div className="form-grid">
              {pinFormMode === 'change' && (
                <label className="form-full-width">
                  <span>{t('currentPinLabel')}</span>
                  <input
                    type="password"
                    inputMode="numeric"
                    autoComplete="current-password"
                    required
                    maxLength={12}
                    placeholder="••••"
                    value={currentPin}
                    onChange={(e) => setCurrentPin(e.target.value)}
                  />
                </label>
              )}
              <label>
                <span>{t('newPinLabel')}</span>
                <input
                  type="password"
                  inputMode="numeric"
                  autoComplete="new-password"
                  required
                  minLength={4}
                  maxLength={12}
                  placeholder="••••"
                  value={newPin}
                  onChange={(e) => setNewPin(e.target.value)}
                />
              </label>
              <label>
                <span>{t('confirmPinLabel')}</span>
                <input
                  type="password"
                  inputMode="numeric"
                  autoComplete="new-password"
                  required
                  minLength={4}
                  maxLength={12}
                  placeholder="••••"
                  value={confirmPin}
                  onChange={(e) => setConfirmPin(e.target.value)}
                />
              </label>
            </div>
            <div className="form-actions-row">
              <button type="button" className="secondary-link" onClick={() => setPinFormMode(null)}>
                {t('cancel')}
              </button>
              <button type="submit" className="primary-link form-submit-btn" disabled={busy}>
                <Lock size={16} />
                <span>{busy ? t('saving') : t('savePinBtn')}</span>
              </button>
            </div>
          </form>
        )}

        {lockStatus?.isPinSet && pinFormMode === 'remove' && (
          <form
            className="account-form-card"
            onSubmit={(e) => void handleRemovePin(e)}
            style={{ marginTop: '1rem' }}
          >
            <div
              className="form-card-header"
              style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
              }}
            >
              <div>
                <h3>{t('removePinTitle')}</h3>
                <p>{t('removePinDesc')}</p>
              </div>
              <button
                type="button"
                className="icon-button"
                aria-label={t('cancel')}
                onClick={() => setPinFormMode(null)}
              >
                <X size={16} />
              </button>
            </div>
            <div className="form-grid">
              <label className="form-full-width">
                <span>{t('currentPinLabel')}</span>
                <input
                  type="password"
                  inputMode="numeric"
                  autoComplete="current-password"
                  required
                  maxLength={12}
                  placeholder="••••"
                  value={currentPin}
                  onChange={(e) => setCurrentPin(e.target.value)}
                />
              </label>
            </div>
            <div className="form-actions-row">
              <button type="button" className="secondary-link" onClick={() => setPinFormMode(null)}>
                {t('cancel')}
              </button>
              <button
                type="submit"
                className="primary-link destructive-btn form-submit-btn"
                disabled={busy}
              >
                <Trash2 size={16} />
                <span>{busy ? t('saving') : t('removePinBtn')}</span>
              </button>
            </div>
          </form>
        )}

        <section className="setting-row-minimal">
          <div className="setting-label-group">
            <div className="setting-icon-box">
              <ShieldCheck size={20} />
            </div>
            <h2 className="setting-title">{t('backup')}</h2>
          </div>
          <div style={{ display: 'flex', gap: '0.5rem' }}>
            <button
              type="button"
              className="glass-pill-btn secondary-link"
              onClick={() => {
                setShowExportForm(!showExportForm);
                setShowImportForm(false);
                setPinFormMode(null);
                setStatusMsg(null);
              }}
            >
              <Download size={14} />
              <span>{t('exportBtn')}</span>
            </button>
            <button
              type="button"
              className="glass-pill-btn secondary-link"
              onClick={() => {
                setShowImportForm(!showImportForm);
                setShowExportForm(false);
                setPinFormMode(null);
                setStatusMsg(null);
              }}
            >
              <Upload size={14} />
              <span>{t('importBtn')}</span>
            </button>
          </div>
        </section>

        {showExportForm && (
          <form
            className="account-form-card"
            onSubmit={(e) => void handleExport(e)}
            style={{ marginTop: '1rem' }}
          >
            <div
              className="form-card-header"
              style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
              }}
            >
              <div>
                <h3>{t('exportModalTitle')}</h3>
                <p>{t('exportModalDesc')}</p>
              </div>
              <button
                type="button"
                className="icon-button"
                aria-label={t('cancel')}
                onClick={() => setShowExportForm(false)}
              >
                <X size={16} />
              </button>
            </div>
            <div className="form-grid">
              <label className="form-full-width">
                <span>{t('passwordLabel')}</span>
                <input
                  type="password"
                  required
                  minLength={12}
                  maxLength={128}
                  placeholder="********"
                  value={exportPassword}
                  onChange={(e) => setExportPassword(e.target.value)}
                />
              </label>
            </div>
            <div className="form-actions-row">
              <button
                type="button"
                className="secondary-link"
                onClick={() => setShowExportForm(false)}
              >
                {t('cancel')}
              </button>
              <button type="submit" className="primary-link form-submit-btn" disabled={busy}>
                <Download size={16} />
                <span>{busy ? t('checking') : t('submitExport')}</span>
              </button>
            </div>
          </form>
        )}

        {showImportForm && (
          <form
            className="account-form-card"
            onSubmit={(e) => void handleImport(e)}
            style={{ marginTop: '1rem' }}
          >
            <div
              className="form-card-header"
              style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
              }}
            >
              <div>
                <h3>{t('importModalTitle')}</h3>
                <p>{t('importModalDesc')}</p>
              </div>
              <button
                type="button"
                className="icon-button"
                aria-label={t('cancel')}
                onClick={() => setShowImportForm(false)}
              >
                <X size={16} />
              </button>
            </div>
            <div className="form-grid">
              <label className="form-full-width">
                <span>{t('passwordLabel')}</span>
                <input
                  type="password"
                  required
                  maxLength={128}
                  placeholder="********"
                  value={importPassword}
                  onChange={(e) => setImportPassword(e.target.value)}
                />
              </label>
            </div>
            <div className="form-actions-row">
              <button
                type="button"
                className="secondary-link"
                onClick={() => setShowImportForm(false)}
              >
                {t('cancel')}
              </button>
              <button type="submit" className="primary-link form-submit-btn" disabled={busy}>
                <Upload size={16} />
                <span>{busy ? t('checking') : t('submitImport')}</span>
              </button>
            </div>
          </form>
        )}

        <section className="setting-row-minimal">
          <div className="setting-label-group">
            <div className="setting-icon-box">
              <RefreshCw size={20} className={checkingUpdate ? 'animate-spin' : undefined} />
            </div>
            <div style={{ display: 'flex', flexDirection: 'column' }}>
              <h2 className="setting-title">{t('updates')}</h2>
              <span style={{ fontSize: '0.78rem', color: '#34d399', fontWeight: 500 }}>
                {updateFailed
                  ? `v${updateState.latestVersion ?? updateInfo?.latestVersion ?? appVersion ?? '—'} (${t(
                      'updateInstallFailed',
                    )})`
                  : downloadingUpdate
                    ? `v${updateState.latestVersion ?? updateInfo?.latestVersion ?? ''} (${t(
                        'downloadingUpdate',
                      )} ${updateProgress}%)`
                    : installingUpdate
                      ? `v${updateState.latestVersion ?? updateInfo?.latestVersion ?? ''} (${t(
                          'installingUpdate',
                        )})`
                      : updateAvailable
                        ? `v${updateState?.latestVersion ?? updateInfo?.latestVersion ?? ''} (${t(
                            'newVersion',
                          )})`
                        : `v${updateInfo?.currentVersion ?? appVersion ?? '—'} (${
                            updateInfo?.error || updateCheckFailed
                              ? t('updateCheckFailed')
                              : updateInfo
                                ? t('upToDate')
                                : t('installed')
                          })`}
              </span>
            </div>
          </div>
          <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem' }}>
            <button
              type="button"
              className={updateAvailable ? 'primary-link' : 'glass-pill-btn secondary-link'}
              disabled={checkingUpdate || downloadingUpdate || installingUpdate}
              onClick={() => void (updateAvailable ? handleInstallUpdate() : handleCheckUpdate())}
            >
              {updateAvailable && <Download size={14} />}
              <span>
                {checkingUpdate
                  ? t('checking')
                  : downloadingUpdate
                    ? `${t('downloadingUpdate')} ${updateProgress}%`
                    : installingUpdate
                      ? t('installingUpdate')
                      : updateAvailable
                        ? updateFailed
                          ? t('retryUpdate')
                          : t('downloadAndInstall')
                        : t('checkUpdateBtn')}
              </span>
            </button>
          </div>
        </section>
      </div>
    </section>
  );
}
