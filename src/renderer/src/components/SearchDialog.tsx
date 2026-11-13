import { ArrowRight, Check, Copy, Plus, Search, Settings } from 'lucide-react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import type { AccountDto } from '@shared/schemas/account';
import type { TotpCode } from '@shared/schemas/totp';
import { useLanguage } from '../hooks/useLanguage';
import {
  filterAndRankAccounts,
  formatTotpCode,
  hasTotpPeriodRolled,
  matchesSearchQuery,
} from '../utils/accountHelpers';

interface SearchDialogProps {
  open: boolean;
  onClose: () => void;
}

type LoadState =
  | { status: 'idle'; accounts: AccountDto[]; codes: TotpCode[] }
  | { status: 'ready'; accounts: AccountDto[]; codes: TotpCode[] }
  | { status: 'error'; accounts: AccountDto[]; codes: TotpCode[] };

const INITIAL_LOAD_STATE: LoadState = { status: 'idle', accounts: [], codes: [] };

export function SearchDialog({ open, onClose }: SearchDialogProps): React.JSX.Element {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const copyTimerRef = useRef<number | null>(null);
  const itemRefs = useRef<(HTMLButtonElement | null)[]>([]);

  const [query, setQuery] = useState('');
  const [loadState, setLoadState] = useState<LoadState>(INITIAL_LOAD_STATE);
  const [copiedAccountId, setCopiedAccountId] = useState<string | null>(null);
  const [copyError, setCopyError] = useState(false);
  const [activeIndex, setActiveIndex] = useState(0);

  const navigate = useNavigate();
  const { t } = useLanguage();

  const actions = useMemo(
    () => [
      { label: t('cmdAddAccount'), path: '/add', icon: Plus },
      { label: t('cmdOpenSettings'), path: '/settings', icon: Settings },
    ],
    [t],
  );

  const clearCopyTimer = useCallback(() => {
    if (copyTimerRef.current !== null) {
      window.clearTimeout(copyTimerRef.current);
      copyTimerRef.current = null;
    }
  }, []);

  const resetDialogState = useCallback(() => {
    clearCopyTimer();
    setQuery('');
    setCopiedAccountId(null);
    setCopyError(false);
    setActiveIndex(0);
    setLoadState(INITIAL_LOAD_STATE);
  }, [clearCopyTimer]);

  const handleClose = useCallback(() => {
    resetDialogState();
    onClose();
  }, [onClose, resetDialogState]);

  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return;

    if (open && !dialog.open) {
      dialog.showModal();
      window.setTimeout(() => inputRef.current?.focus(), 0);
    } else if (!open && dialog.open) {
      dialog.close();
    }
  }, [open]);

  useEffect(() => {
    if (!open) {
      const resetTimer = window.setTimeout(() => resetDialogState(), 0);
      return () => window.clearTimeout(resetTimer);
    }

    let cancelled = false;
    void Promise.all([
      window.authapp.listAccounts(),
      window.authapp.getTotpCodes().catch(() => [] as TotpCode[]),
    ])
      .then(([accounts, codes]) => {
        if (!cancelled) {
          setLoadState({ status: 'ready', accounts, codes });
        }
      })
      .catch(() => {
        if (!cancelled) {
          setLoadState({ status: 'error', accounts: [], codes: [] });
        }
      });

    return () => {
      cancelled = true;
    };
  }, [open, resetDialogState]);

  useEffect(() => {
    if (!open || loadState.status !== 'ready' || loadState.accounts.length === 0) return;

    let cancelled = false;
    let lastSec = Math.floor(Date.now() / 1000);

    const refreshTimer = window.setInterval(() => {
      const currentSec = Math.floor(Date.now() / 1000);
      if (hasTotpPeriodRolled(loadState.accounts, lastSec, currentSec)) {
        void window.authapp
          .getTotpCodes()
          .then((nextCodes) => {
            if (!cancelled) {
              setLoadState((prev) =>
                prev.status === 'ready' ? { ...prev, codes: nextCodes } : prev,
              );
            }
          })
          .catch(() => {});
      }
      lastSec = currentSec;
    }, 1000);

    return () => {
      cancelled = true;
      window.clearInterval(refreshTimer);
    };
  }, [open, loadState.status, loadState.accounts]);

  useEffect(() => clearCopyTimer, [clearCopyTimer]);

  const codeByAccountId = useMemo(
    () => new Map(loadState.codes.map((code) => [code.accountId, code])),
    [loadState.codes],
  );

  const filteredAccounts = useMemo(
    () => filterAndRankAccounts(loadState.accounts, query),
    [loadState.accounts, query],
  );

  const filteredActions = useMemo(
    () => actions.filter((action) => matchesSearchQuery(action.label, query)),
    [actions, query],
  );

  const totalItems = filteredAccounts.length + filteredActions.length;
  const safeActiveIndex = totalItems > 0 ? Math.min(activeIndex, totalItems - 1) : -1;

  const choose = useCallback(
    (path: string): void => {
      resetDialogState();
      onClose();
      navigate(path);
    },
    [navigate, onClose, resetDialogState],
  );

  const handleJumpToAccount = useCallback(
    (accountId: string): void => {
      resetDialogState();
      onClose();
      navigate('/', { state: { focusAccountId: accountId, focusNonce: Date.now() } });
    },
    [navigate, onClose, resetDialogState],
  );

  const handleCopyAccount = useCallback(
    async (accountId: string): Promise<void> => {
      try {
        setCopyError(false);
        await window.authapp.copyTotp(accountId);
      } catch {
        setCopyError(true);
        return;
      }

      clearCopyTimer();
      setCopiedAccountId(accountId);
      copyTimerRef.current = window.setTimeout(() => {
        setCopiedAccountId((prev) => (prev === accountId ? null : prev));
        copyTimerRef.current = null;
      }, 1800);
    },
    [clearCopyTimer],
  );

  const handleInputKeyDown = (event: React.KeyboardEvent<HTMLInputElement>): void => {
    if (totalItems === 0) return;

    if (event.key === 'ArrowDown') {
      event.preventDefault();
      const nextIndex = (safeActiveIndex + 1) % totalItems;
      setActiveIndex(nextIndex);
      itemRefs.current[nextIndex]?.scrollIntoView?.({ block: 'nearest' });
    } else if (event.key === 'ArrowUp') {
      event.preventDefault();
      const nextIndex = (safeActiveIndex - 1 + totalItems) % totalItems;
      setActiveIndex(nextIndex);
      itemRefs.current[nextIndex]?.scrollIntoView?.({ block: 'nearest' });
    } else if (event.key === 'Enter' && safeActiveIndex >= 0) {
      event.preventDefault();
      if (safeActiveIndex < filteredAccounts.length) {
        const account = filteredAccounts[safeActiveIndex];
        if (!account) return;
        if (codeByAccountId.has(account.id)) {
          void handleCopyAccount(account.id);
        } else {
          handleJumpToAccount(account.id);
        }
      } else {
        const action = filteredActions[safeActiveIndex - filteredAccounts.length];
        if (action) choose(action.path);
      }
    }
  };

  const trimmedQuery = query.trim();
  const isLoading = open && loadState.status === 'idle';

  return (
    <dialog
      ref={dialogRef}
      className="search-dialog spotlight-liquid-glass"
      aria-labelledby="search-title"
      onCancel={(event) => {
        event.preventDefault();
        handleClose();
      }}
      onClose={() => {
        if (open) handleClose();
      }}
      onKeyDown={(event) => {
        if (event.key === 'Escape') {
          event.preventDefault();
          handleClose();
        }
      }}
      onClick={(event) => {
        if (event.target === dialogRef.current) handleClose();
      }}
    >
      <div className="search-dialog__panel">
        <label className="search-field" htmlFor="account-search">
          <Search size={19} aria-hidden="true" />
          <span className="sr-only" id="search-title">
            {t('navSearch')}
          </span>
          <input
            ref={inputRef}
            id="account-search"
            value={query}
            onChange={(event) => {
              setQuery(event.target.value);
              setActiveIndex(0);
              setCopyError(false);
            }}
            onKeyDown={handleInputKeyDown}
            placeholder={t('searchPlaceholder')}
            autoComplete="off"
          />
          <kbd>Esc</kbd>
        </label>

        <div className="search-results" aria-live="polite">
          {copyError ? (
            <p className="search-results__error" role="alert">
              {t('searchCopyError')}
            </p>
          ) : null}

          {isLoading ? (
            <p className="search-results__meta">{t('searchLoading')}</p>
          ) : (
            <>
              {loadState.status === 'error' ? (
                <p className="search-results__error" role="alert">
                  {t('searchLoadError')}
                </p>
              ) : loadState.accounts.length === 0 && !trimmedQuery ? (
                <p className="search-results__meta">{t('searchNoAccounts')}</p>
              ) : null}

              {filteredAccounts.length > 0 ? (
                <div className="search-results__section">
                  <p className="search-results__meta">
                    {t('searchAccountsSection')} ({filteredAccounts.length})
                  </p>
                  {filteredAccounts.map((account, index) => {
                    const value = codeByAccountId.get(account.id);
                    const isCopied = copiedAccountId === account.id;
                    const isActive = safeActiveIndex === index;

                    return (
                      <div className="search-account-row" key={account.id}>
                        <button
                          ref={(element) => {
                            itemRefs.current[index] = element;
                          }}
                          type="button"
                          className={`search-account-btn ${isActive ? 'is-active' : ''}`}
                          disabled={!value}
                          onClick={() => void handleCopyAccount(account.id)}
                          onMouseEnter={() => setActiveIndex(index)}
                        >
                          <span
                            className="service-avatar search-account-avatar liquid-glass-pill"
                            aria-hidden="true"
                          >
                            {account.issuer.slice(0, 2).toUpperCase()}
                          </span>
                          <span className="search-account-info">
                            <strong className="search-account-issuer">{account.issuer}</strong>
                            <span className="search-account-name">{account.accountName}</span>
                          </span>
                          <span className={`search-account-code ${value ? '' : 'is-unavailable'}`}>
                            {value ? formatTotpCode(value.code) : t('codeUnavailable')}
                          </span>
                          <span
                            className={`search-account-copy liquid-glass-pill ${isCopied ? 'is-copied' : ''}`}
                          >
                            {isCopied ? (
                              <>
                                <Check size={14} aria-hidden="true" />
                                <span>{t('copied')}</span>
                              </>
                            ) : (
                              <>
                                <Copy size={14} aria-hidden="true" />
                                <span>{t('copy')}</span>
                              </>
                            )}
                          </span>
                        </button>
                        <button
                          type="button"
                          className="search-account-goto"
                          aria-label={`${account.issuer} - ${t('searchShowAccount')}`}
                          title={t('searchShowAccount')}
                          onClick={() => handleJumpToAccount(account.id)}
                        >
                          <ArrowRight size={16} aria-hidden="true" />
                        </button>
                      </div>
                    );
                  })}
                </div>
              ) : null}

              {filteredActions.length > 0 ? (
                <div className="search-results__section">
                  {loadState.accounts.length > 0 || trimmedQuery ? (
                    <p className="search-results__meta">{t('searchCommandsSection')}</p>
                  ) : null}
                  {filteredActions.map(({ label, path, icon: Icon }, actionIndex) => {
                    const itemIndex = filteredAccounts.length + actionIndex;
                    const isActive = safeActiveIndex === itemIndex;

                    return (
                      <button
                        ref={(element) => {
                          itemRefs.current[itemIndex] = element;
                        }}
                        key={path}
                        type="button"
                        className={`search-command-btn ${isActive ? 'is-active' : ''}`}
                        onClick={() => choose(path)}
                        onMouseEnter={() => setActiveIndex(itemIndex)}
                      >
                        <Icon size={18} aria-hidden="true" />
                        <span>{label}</span>
                        <ArrowRight size={17} aria-hidden="true" />
                      </button>
                    );
                  })}
                </div>
              ) : null}

              {filteredAccounts.length === 0 && filteredActions.length === 0 ? (
                <p className="search-results__empty">{t('searchNoMatch')}</p>
              ) : null}
            </>
          )}
        </div>
      </div>
    </dialog>
  );
}
