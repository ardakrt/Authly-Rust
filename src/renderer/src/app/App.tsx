import { useCallback, useEffect, useState } from 'react';
import type { LockStatus } from '@shared/schemas/lock';
import { Navigate, Route, Routes } from 'react-router-dom';
import { AppHeader } from '../components/AppHeader';
import { SearchDialog } from '../components/SearchDialog';
import { LockScreenOverlay } from '../components/LockScreenOverlay';
import { UpdateToast } from '../components/UpdateToast';
import { useKeyboardShortcuts } from '../hooks/useKeyboardShortcuts';
import { AddAccountPage } from '../pages/AddAccountPage';
import { HomePage } from '../pages/HomePage';
import { SettingsPage } from '../pages/SettingsPage';

export function App(): React.JSX.Element {
  const [searchOpen, setSearchOpen] = useState(false);
  const [unlocked, setUnlocked] = useState(false);
  const handleLockStatus = useCallback((status: LockStatus) => {
    setUnlocked(!status.isLocked);
    if (status.isLocked) setSearchOpen(false);
  }, []);
  const openSearch = useCallback(() => {
    if (unlocked) setSearchOpen(true);
  }, [unlocked]);
  const closeSearch = useCallback(() => setSearchOpen(false), []);
  useKeyboardShortcuts(openSearch);

  useEffect(() => {
    if (!unlocked) return;
    let lastActivity = 0;
    const recordActivity = () => {
      if (Date.now() - lastActivity < 1000) return;
      lastActivity = Date.now();
      void window.authapp.recordActivity().catch(() => {});
    };
    const events = ['pointerdown', 'pointermove', 'keydown', 'wheel', 'focus'] as const;
    for (const event of events) window.addEventListener(event, recordActivity, true);
    return () => {
      for (const event of events) window.removeEventListener(event, recordActivity, true);
    };
  }, [unlocked]);

  return (
    <div className="app-wrapper app-shell">
      <LockScreenOverlay onStatusChange={handleLockStatus} />

      <div className="dark-horizon-glow" aria-hidden="true" />

      {unlocked && (
        <>
          <AppHeader onOpenSearch={openSearch} />

          <main className="app-main" id="main-content">
            <Routes>
              <Route path="/" element={<HomePage />} />
              <Route path="/add" element={<AddAccountPage />} />
              <Route path="/settings" element={<SettingsPage />} />
              <Route path="*" element={<Navigate to="/" replace />} />
            </Routes>
          </main>

          <SearchDialog open={searchOpen} onClose={closeSearch} />
          <UpdateToast />
        </>
      )}
    </div>
  );
}
