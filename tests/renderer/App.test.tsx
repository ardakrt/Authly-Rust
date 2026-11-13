import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { App } from '../../src/renderer/src/app/App';
import { LanguageProvider } from '../../src/renderer/src/app/LanguageProvider';
import { ThemeProvider } from '../../src/renderer/src/app/ThemeProvider';
import { translations } from '../../src/renderer/src/utils/translations';

function renderApp(initialEntries = ['/']): void {
  render(
    <MemoryRouter initialEntries={initialEntries}>
      <ThemeProvider>
        <App />
      </ThemeProvider>
    </MemoryRouter>,
  );
}

describe('App shell and user interface', () => {
  beforeEach(() => {
    window.localStorage.clear();
    vi.clearAllMocks();
    vi.mocked(window.authapp.getSettings).mockResolvedValue({
      theme: 'system',
      language: 'tr',
      closeToTray: true,
      startMinimized: false,
    });
    vi.mocked(window.authapp.getLockStatus).mockResolvedValue({
      isPinSet: false,
      isLocked: false,
      autoLockTimeout: 5,
    });
  });

  it('uses the packaged Authly logo in the application header', async () => {
    renderApp();

    const brandButton = await screen.findByRole('button', { name: 'Authly Ana Sayfası' });
    expect(brandButton.querySelector('img')).toHaveAttribute(
      'src',
      expect.stringContaining('logo.png'),
    );
  });

  it('shows an empty state when no accounts exist', async () => {
    renderApp();

    expect(await screen.findByRole('heading', { name: 'Henüz hesap eklenmedi' })).toBeVisible();
    expect(screen.queryByText(/\b\d{3}\s\d{3}\b/)).not.toBeInTheDocument();
  });

  it('supports the settings and search keyboard shortcuts', async () => {
    const user = userEvent.setup();
    renderApp();

    await user.keyboard('{Control>},{/Control}');
    expect(await screen.findByRole('heading', { name: 'Ayarlar' })).toBeVisible();

    await user.keyboard('{Control>}f{/Control}');
    await waitFor(() => expect(screen.getByRole('dialog')).toHaveAttribute('open'));
    expect(screen.getByPlaceholderText('Hesap veya komut ara')).toHaveFocus();
  });

  it('changes the selected theme using settings and persists it', async () => {
    const user = userEvent.setup();
    renderApp(['/settings']);

    await user.click(await screen.findByRole('button', { name: 'Koyu' }));
    expect(document.documentElement).toHaveAttribute('data-theme', 'dark');
    expect(screen.getByRole('button', { name: 'Koyu' })).toHaveAttribute('aria-pressed', 'true');
    expect(window.localStorage.getItem('authapp.theme')).toBe('dark');
    expect(window.authapp.updateSettings).toHaveBeenCalledWith({ theme: 'dark' });
  });

  it('loads the saved theme from backend settings when localStorage has no theme', async () => {
    vi.mocked(window.authapp.getSettings).mockResolvedValue({
      theme: 'dark',
      language: 'tr',
      closeToTray: true,
      startMinimized: false,
    });
    renderApp(['/settings']);

    await waitFor(() => {
      expect(document.documentElement).toHaveAttribute('data-theme', 'dark');
    });
    expect(window.localStorage.getItem('authapp.theme')).toBe('dark');
    expect(screen.getByRole('button', { name: 'Koyu' })).toHaveAttribute('aria-pressed', 'true');
  });

  it('shows the installed package version in settings', async () => {
    renderApp(['/settings']);

    expect(await screen.findByText('v0.3.5 (Kurulu)')).toBeVisible();
  });

  it('creates a Master PIN from settings', async () => {
    const user = userEvent.setup();
    renderApp(['/settings']);

    await user.click(await screen.findByRole('button', { name: 'PIN Oluştur' }));
    await user.type(screen.getByLabelText('Yeni PIN (4–12 karakter)'), '1234');
    await user.type(screen.getByLabelText('Yeni PIN (Tekrar)'), '1234');
    await user.click(screen.getByRole('button', { name: 'PIN Kaydet' }));

    await waitFor(() => {
      expect(window.authapp.setPin).toHaveBeenCalledWith({ pin: '1234' });
    });
    expect(await screen.findByText('Master PIN başarıyla oluşturuldu.')).toBeVisible();
    expect(screen.getByRole('button', { name: 'PIN Değiştir' })).toBeVisible();
    expect(screen.getByRole('button', { name: 'PIN Kaldır' })).toBeVisible();
    expect(screen.getByRole('button', { name: 'Şimdi Kilitle' })).toBeVisible();
  });

  it('changes, locks, and removes an existing Master PIN from settings', async () => {
    const user = userEvent.setup();
    vi.mocked(window.authapp.getLockStatus).mockResolvedValue({
      isPinSet: true,
      isLocked: false,
      autoLockTimeout: 5,
    });
    renderApp(['/settings']);

    await user.click(await screen.findByRole('button', { name: 'PIN Değiştir' }));
    await user.type(screen.getByLabelText('Mevcut PIN'), '1234');
    await user.type(screen.getByLabelText('Yeni PIN (4–12 karakter)'), '5678');
    await user.type(screen.getByLabelText('Yeni PIN (Tekrar)'), '5678');
    await user.click(screen.getByRole('button', { name: 'PIN Kaydet' }));

    await waitFor(() => {
      expect(window.authapp.setPin).toHaveBeenCalledWith({ pin: '5678', currentPin: '1234' });
    });
    expect(await screen.findByText('Master PIN başarıyla güncellendi.')).toBeVisible();

    await user.click(screen.getByRole('button', { name: 'Şimdi Kilitle' }));
    await waitFor(() => {
      expect(window.authapp.lockApp).toHaveBeenCalledTimes(1);
    });

    await user.click(screen.getByRole('button', { name: 'PIN Kaldır' }));
    await user.type(screen.getByLabelText('Mevcut PIN'), '5678');
    const removeSubmitButtons = screen.getAllByRole('button', { name: 'PIN Kaldır' });
    const submitBtn = removeSubmitButtons[removeSubmitButtons.length - 1];
    if (!submitBtn) throw new Error('Expected remove submit button');
    await user.click(submitBtn);

    await waitFor(() => {
      expect(window.authapp.removePin).toHaveBeenCalledWith({ currentPin: '5678' });
    });
    expect(await screen.findByText('Master PIN kaldırıldı.')).toBeVisible();
  });

  it('provides full English localization parity across settings and translations', async () => {
    expect(Object.keys(translations.en).sort()).toEqual(Object.keys(translations.tr).sort());

    window.localStorage.setItem('authapp.language', 'en');
    vi.mocked(window.authapp.getSettings).mockResolvedValue({
      theme: 'system',
      language: 'en',
      closeToTray: true,
      startMinimized: false,
    });

    render(
      <MemoryRouter initialEntries={['/settings']}>
        <LanguageProvider>
          <ThemeProvider>
            <App />
          </ThemeProvider>
        </LanguageProvider>
      </MemoryRouter>,
    );

    expect(await screen.findByRole('heading', { name: 'Master PIN Lock' })).toBeVisible();
    expect(screen.getByRole('button', { name: 'Set PIN' })).toBeVisible();
  });
});
