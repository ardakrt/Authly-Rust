import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { ThemeContext, type ThemePreference } from './themeContext';

const STORAGE_KEY = 'authapp.theme';

function readPreference(): ThemePreference {
  const stored = window.localStorage.getItem(STORAGE_KEY);
  return stored === 'light' || stored === 'dark' || stored === 'system' ? stored : 'system';
}

function getResolvedTheme(preference: ThemePreference): 'light' | 'dark' {
  if (preference !== 'system') return preference;
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

export function ThemeProvider({ children }: { children: ReactNode }): React.JSX.Element {
  const [preference, setPreferenceState] = useState<ThemePreference>(readPreference);
  const userModifiedRef = useRef(false);

  const applyTheme = useCallback((nextPreference: ThemePreference) => {
    document.documentElement.dataset['theme'] = getResolvedTheme(nextPreference);
  }, []);

  const setPreference = useCallback(
    (nextPreference: ThemePreference) => {
      userModifiedRef.current = true;
      window.localStorage.setItem(STORAGE_KEY, nextPreference);
      setPreferenceState(nextPreference);
      applyTheme(nextPreference);
      void window.authapp?.updateSettings?.({ theme: nextPreference })?.catch(() => {});
    },
    [applyTheme],
  );

  useEffect(() => {
    if (window.localStorage.getItem(STORAGE_KEY) !== null) return;
    let cancelled = false;
    void window.authapp
      ?.getSettings?.()
      ?.then((s) => {
        if (cancelled || userModifiedRef.current) return;
        if (s?.theme === 'light' || s?.theme === 'dark' || s?.theme === 'system') {
          window.localStorage.setItem(STORAGE_KEY, s.theme);
          setPreferenceState(s.theme);
          applyTheme(s.theme);
        }
      })
      ?.catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [applyTheme]);

  useEffect(() => {
    applyTheme(preference);
    const media = window.matchMedia('(prefers-color-scheme: dark)');
    const handleChange = (): void => {
      if (preference === 'system') applyTheme(preference);
    };
    media.addEventListener('change', handleChange);
    return () => media.removeEventListener('change', handleChange);
  }, [applyTheme, preference]);

  const value = useMemo(() => ({ preference, setPreference }), [preference, setPreference]);
  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}
