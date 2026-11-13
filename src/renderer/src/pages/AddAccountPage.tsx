import {
  ArrowLeft,
  CheckCircle2,
  ChevronDown,
  ChevronUp,
  Clipboard,
  KeyRound,
  Layers,
  RefreshCw,
  Save,
  Upload,
} from 'lucide-react';
import { useCallback, useEffect, useState, type FormEvent } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import type { CreateAccountRequest } from '@shared/schemas/account';
import { createAccountRequestSchema } from '@shared/schemas/account';
import { PageHeading } from '../components/PageHeading';
import { decodeQrFromImageFile, processScannedQrContent } from '../utils/qrDecoder';
import { useLanguage } from '../hooks/useLanguage';

type AddMode = 'upload' | 'manual';

const defaultForm: CreateAccountRequest = {
  issuer: '',
  accountName: '',
  secret: '',
  algorithm: 'SHA1',
  digits: 6,
  period: 30,
  favorite: false,
  groupId: null,
};

const popularServices = [
  'Google',
  'GitHub',
  'Microsoft',
  'Discord',
  'Binance',
  'Instagram',
  'Amazon',
  'Steam',
];

export function AddAccountPage(): React.JSX.Element {
  const navigate = useNavigate();
  const { t } = useLanguage();
  const [mode, setMode] = useState<AddMode>('upload');
  const [form, setForm] = useState<CreateAccountRequest>(defaultForm);
  const [batchAccounts, setBatchAccounts] = useState<
    Array<{ account: CreateAccountRequest; selected: boolean }>
  >([]);
  const [error, setError] = useState<string>('');
  const [successMsg, setSuccessMsg] = useState<string>('');
  const [saving, setSaving] = useState<boolean>(false);
  const [showAdvanced, setShowAdvanced] = useState<boolean>(false);
  const [scannedPreview, setScannedPreview] = useState<boolean>(false);

  // Handle scanned QR string
  const handleDecodedContent = useCallback(
    async (content: string) => {
      try {
        const parsedList = await processScannedQrContent(content);

        if (parsedList.length === 1 && parsedList[0]) {
          setForm(parsedList[0]);
          setBatchAccounts([]);
          setScannedPreview(true);
          setError('');
          setSuccessMsg(t('qrDetectedSingle'));
        } else if (parsedList.length > 1) {
          setBatchAccounts(parsedList.map((account) => ({ account, selected: true })));
          setScannedPreview(true);
          setError('');
          setSuccessMsg(t('qrDetectedBatch').replace('{count}', String(parsedList.length)));
        }
      } catch (err) {
        setError(err instanceof Error ? err.message : t('qrReadError'));
      }
    },
    [t],
  );

  // Handle clipboard paste (Ctrl+V or clipboard image/text)
  const handlePaste = useCallback(
    async (clipboardData?: DataTransfer) => {
      setError('');
      setSuccessMsg('');
      try {
        let fileToProcess: File | null = null;

        if (clipboardData && clipboardData.files && clipboardData.files.length > 0) {
          fileToProcess = clipboardData.files[0] ?? null;
        }

        if (!fileToProcess && navigator.clipboard && navigator.clipboard.read) {
          try {
            const items = await navigator.clipboard.read();
            for (const item of items) {
              const imageType = item.types.find((t) => t.startsWith('image/'));
              if (imageType) {
                const blob = await item.getType(imageType);
                fileToProcess = new File([blob], 'pasted-qr.png', { type: imageType });
                break;
              }
            }
          } catch {
            // Ignore permission / clip fallback
          }
        }

        if (fileToProcess) {
          const content = await decodeQrFromImageFile(fileToProcess);
          await handleDecodedContent(content);
          return;
        }

        const text = clipboardData
          ? clipboardData.getData('text')
          : await navigator.clipboard.readText();
        if (text && text.trim()) {
          await handleDecodedContent(text.trim());
          return;
        }

        setError(t('clipboardEmptyError'));
      } catch (err) {
        setError(err instanceof Error ? err.message : t('clipboardReadError'));
      }
    },
    [handleDecodedContent, t],
  );

  // Global paste event listener when upload mode is active
  useEffect(() => {
    const onWindowPaste = (e: ClipboardEvent) => {
      if (mode === 'upload' && e.clipboardData) {
        void handlePaste(e.clipboardData);
      }
    };
    window.addEventListener('paste', onWindowPaste);
    return () => window.removeEventListener('paste', onWindowPaste);
  }, [mode, handlePaste]);

  // Handle File Input Change
  const handleFileChange = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    setError('');
    setSuccessMsg('');
    try {
      const content = await decodeQrFromImageFile(file);
      await handleDecodedContent(content);
    } catch (err) {
      setError(err instanceof Error ? err.message : t('fileReadError'));
    }
  };

  // Submit single account form
  async function submitSingleForm(event: FormEvent): Promise<void> {
    event.preventDefault();
    if (!form.issuer.trim()) {
      setError(t('serviceRequiredError'));
      return;
    }
    if (!form.accountName.trim()) {
      setError(t('accountNameRequiredError'));
      return;
    }
    const parsed = createAccountRequestSchema.safeParse(form);
    if (!parsed.success) {
      setError(t('invalidAccountInput'));
      return;
    }

    setSaving(true);
    setError('');
    try {
      await window.authapp.createAccount(parsed.data);
      navigate('/');
    } catch {
      setError(t('accountSaveError'));
      setSaving(false);
    }
  }

  // Submit batch import
  async function submitBatch(): Promise<void> {
    const selectedList = batchAccounts.filter((item) => item.selected).map((item) => item.account);
    if (selectedList.length === 0) {
      setError(t('selectAtLeastOneAccountError'));
      return;
    }

    setSaving(true);
    setError('');
    try {
      await window.authapp.importAccounts({ accounts: selectedList });
      navigate('/');
    } catch {
      setError(t('accountImportFailed'));
      setSaving(false);
    }
  }

  const toggleBatchSelectAll = (selectAll: boolean) => {
    setBatchAccounts(batchAccounts.map((item) => ({ ...item, selected: selectAll })));
  };

  const toggleBatchAccount = (index: number) => {
    setBatchAccounts(
      batchAccounts.map((item, i) => (i === index ? { ...item, selected: !item.selected } : item)),
    );
  };

  const selectedBatchCount = batchAccounts.filter((item) => item.selected).length;

  return (
    <section className="workspace add-account-flow">
      <PageHeading
        title={t('addNewAccount')}
        description={t('addSubtitle')}
        action={
          <Link className="secondary-link" to="/">
            <ArrowLeft size={18} /> {t('backHome')}
          </Link>
        }
      />

      <div className="add-mode-tabs" role="tablist" aria-label={t('addMethodAriaLabel')}>
        <button
          type="button"
          role="tab"
          aria-selected={mode === 'upload'}
          className={`tab-btn ${mode === 'upload' ? 'is-active' : ''}`}
          onClick={() => {
            setMode('upload');
            setError('');
            setScannedPreview(false);
            setBatchAccounts([]);
          }}
        >
          <Upload size={18} />
          <span>{t('tabUpload')}</span>
        </button>

        <button
          type="button"
          role="tab"
          aria-selected={mode === 'manual'}
          className={`tab-btn ${mode === 'manual' ? 'is-active' : ''}`}
          onClick={() => {
            setMode('manual');
            setError('');
            setScannedPreview(false);
            setBatchAccounts([]);
          }}
        >
          <KeyRound size={18} />
          <span>{t('tabManual')}</span>
        </button>
      </div>

      {error ? (
        <div className="error-banner" role="alert">
          {error}
        </div>
      ) : null}

      {successMsg ? (
        <div className="success-banner" role="status">
          <CheckCircle2 size={18} />
          <span>{successMsg}</span>
        </div>
      ) : null}

      {mode === 'upload' && !scannedPreview && batchAccounts.length === 0 && (
        <div className="upload-container">
          <label className="drag-drop-zone">
            <Upload size={40} className="upload-icon" />
            <span className="upload-title">{t('dragDropTitle')}</span>
            <span className="upload-subtitle">{t('dragDropSub')}</span>
            <input
              type="file"
              accept="image/*"
              className="sr-only"
              onChange={(e) => void handleFileChange(e)}
            />
          </label>

          <div className="or-divider">
            <span>{t('orDivider')}</span>
          </div>

          <button
            type="button"
            className="secondary-link paste-clipboard-btn"
            onClick={() => void handlePaste()}
          >
            <Clipboard size={18} />
            <span>{t('pasteClipboard')}</span>
            <kbd>Ctrl+V</kbd>
          </button>
        </div>
      )}

      {batchAccounts.length > 1 && (
        <div className="account-form-card">
          <div className="form-card-header">
            <h3>
              <Layers size={20} style={{ display: 'inline', marginRight: '0.5rem' }} />
              {t('batchImportTitle').replace('{count}', String(batchAccounts.length))}
            </h3>
            <p>{t('batchImportDesc')}</p>
          </div>

          <div className="batch-actions-header">
            <button
              type="button"
              className="chip-btn"
              onClick={() => toggleBatchSelectAll(selectedBatchCount < batchAccounts.length)}
            >
              {selectedBatchCount < batchAccounts.length ? t('selectAll') : t('deselectAll')}
            </button>
            <span className="chips-label">
              {t('batchSelectedCount')
                .replace('{selected}', String(selectedBatchCount))
                .replace('{total}', String(batchAccounts.length))}
            </span>
          </div>

          <div className="batch-account-list">
            {batchAccounts.map((item, idx) => (
              <label key={idx} className={`batch-item ${item.selected ? 'is-selected' : ''}`}>
                <input
                  type="checkbox"
                  checked={item.selected}
                  onChange={() => toggleBatchAccount(idx)}
                />
                <div className="batch-item-info">
                  <strong>{item.account.issuer}</strong>
                  <span>{item.account.accountName}</span>
                </div>
                <div className="batch-item-secret">
                  <code>•••• {item.account.secret.slice(-4)}</code>
                </div>
              </label>
            ))}
          </div>

          <div className="form-actions-row">
            <button
              type="button"
              className="secondary-link"
              onClick={() => {
                setBatchAccounts([]);
                setScannedPreview(false);
                setSuccessMsg('');
              }}
            >
              <RefreshCw size={16} /> {t('cancel')}
            </button>

            <button
              type="button"
              className="primary-link form-submit-btn"
              disabled={saving || selectedBatchCount === 0}
              onClick={() => void submitBatch()}
            >
              <Save size={18} />
              <span>
                {saving
                  ? t('saving')
                  : t('saveSelectedAccounts').replace('{count}', String(selectedBatchCount))}
              </span>
            </button>
          </div>
        </div>
      )}

      {(mode === 'manual' || (scannedPreview && batchAccounts.length <= 1)) && (
        <form className="account-form-card" onSubmit={(e) => void submitSingleForm(e)}>
          <div className="form-card-header">
            <h3>{scannedPreview ? t('detectedAccountDetails') : t('accountDetails')}</h3>
            <p>{t('confirmAccountDetails')}</p>
          </div>

          {mode === 'manual' && (
            <div className="service-chips">
              <span className="chips-label">{t('popularServices')}</span>
              <div className="chips-list">
                {popularServices.map((srv) => (
                  <button
                    type="button"
                    key={srv}
                    className={`chip-btn ${form.issuer.toLowerCase() === srv.toLowerCase() ? 'is-selected' : ''}`}
                    onClick={() => setForm({ ...form, issuer: srv })}
                  >
                    {srv}
                  </button>
                ))}
              </div>
            </div>
          )}

          <div className="form-grid">
            <label>
              <span>{t('serviceName')}</span>
              <input
                required
                maxLength={120}
                placeholder={t('servicePlaceholder')}
                value={form.issuer}
                onChange={(e) => setForm({ ...form, issuer: e.target.value })}
              />
            </label>

            <label>
              <span>{t('accountNameEmail')}</span>
              <input
                required
                maxLength={240}
                placeholder={t('accountNamePlaceholder')}
                value={form.accountName}
                onChange={(e) => setForm({ ...form, accountName: e.target.value })}
              />
            </label>

            <label className="form-full-width">
              <span>{t('secretKey')}</span>
              <input
                required
                autoComplete="off"
                spellCheck={false}
                placeholder={t('secretKeyPlaceholder')}
                value={form.secret}
                onChange={(e) => setForm({ ...form, secret: e.target.value.replace(/\s+/g, '') })}
              />
            </label>
          </div>

          <div className="advanced-accordion">
            <button
              type="button"
              className="advanced-toggle-btn"
              onClick={() => setShowAdvanced(!showAdvanced)}
            >
              <span>{t('advancedOptions')}</span>
              {showAdvanced ? <ChevronUp size={16} /> : <ChevronDown size={16} />}
            </button>

            {showAdvanced && (
              <div className="advanced-fields-grid">
                <label>
                  <span>{t('algorithm')}</span>
                  <select
                    value={form.algorithm}
                    onChange={(e) =>
                      setForm({
                        ...form,
                        algorithm: e.target.value as CreateAccountRequest['algorithm'],
                      })
                    }
                  >
                    <option value="SHA1">SHA-1</option>
                    <option value="SHA256">SHA-256</option>
                    <option value="SHA512">SHA-512</option>
                  </select>
                </label>

                <label>
                  <span>{t('digits')}</span>
                  <input
                    type="number"
                    min="6"
                    max="10"
                    value={form.digits}
                    onChange={(e) => setForm({ ...form, digits: Number(e.target.value) })}
                  />
                </label>

                <label>
                  <span>{t('period')}</span>
                  <input
                    type="number"
                    min="5"
                    max="300"
                    value={form.period}
                    onChange={(e) => setForm({ ...form, period: Number(e.target.value) })}
                  />
                </label>
              </div>
            )}
          </div>

          <div className="form-actions-row">
            {scannedPreview && (
              <button
                type="button"
                className="secondary-link"
                onClick={() => {
                  setScannedPreview(false);
                  setForm(defaultForm);
                }}
              >
                <RefreshCw size={16} /> {t('uploadAgain')}
              </button>
            )}

            <button className="primary-link form-submit-btn" disabled={saving} type="submit">
              <Save size={18} />
              <span>{saving ? t('saving') : t('saveAccount')}</span>
            </button>
          </div>
        </form>
      )}
    </section>
  );
}
