import { act, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { App } from '../../src/renderer/src/app/App';
import { LanguageProvider } from '../../src/renderer/src/app/LanguageProvider';
import { ThemeProvider } from '../../src/renderer/src/app/ThemeProvider';
import type { AccountDto } from '../../src/shared/schemas/account';

const discordAccount: AccountDto = {
  id: 'a4ff59c6-2df2-4136-86ba-78bd57a617d8',
  issuer: 'Discord',
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

const instagramAccount: AccountDto = {
  id: '810dc456-3f63-4dc4-b943-bf272fba2cfa',
  issuer: 'Instagram',
  accountName: 'arda.photo',
  algorithm: 'SHA1',
  digits: 6,
  period: 30,
  favorite: false,
  groupId: null,
  sortOrder: 1,
  createdAt: 2,
  updatedAt: 2,
};

const brokenAccount: AccountDto = {
  id: '910dc456-3f63-4dc4-b943-bf272fba2cfb',
  issuer: 'GitHub',
  accountName: 'broken@example.com',
  algorithm: 'SHA1',
  digits: 6,
  period: 30,
  favorite: false,
  groupId: null,
  sortOrder: 2,
  createdAt: 3,
  updatedAt: 3,
};

function renderApp(initialEntries = ['/']) {
  return render(
    <MemoryRouter initialEntries={initialEntries}>
      <LanguageProvider>
        <ThemeProvider>
          <App />
        </ThemeProvider>
      </LanguageProvider>
    </MemoryRouter>,
  );
}

describe('SearchDialog account search', () => {
  beforeEach(() => {
    window.localStorage.setItem('authapp.language', 'tr');
    vi.mocked(window.authapp.getLockStatus).mockResolvedValue({
      isPinSet: false,
      isLocked: false,
      autoLockTimeout: 5,
    });
    vi.mocked(window.authapp.onLockStatus).mockReturnValue(() => {});
    vi.mocked(window.authapp.getSettings).mockResolvedValue({
      theme: 'system',
      language: 'tr',
      closeToTray: true,
      startMinimized: false,
    });
    vi.mocked(window.authapp.listAccounts).mockResolvedValue([discordAccount, instagramAccount]);
    vi.mocked(window.authapp.getTotpCodes).mockResolvedValue([
      { accountId: discordAccount.id, code: '482913', remaining: 15, period: 30 },
      { accountId: instagramAccount.id, code: '731042', remaining: 15, period: 30 },
    ]);
    vi.mocked(window.authapp.copyTotp).mockResolvedValue(undefined);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('loads and displays saved accounts with live TOTP codes instead of "Kayıtlı hesap yok"', async () => {
    const user = userEvent.setup();
    renderApp();

    await screen.findByText('Discord');
    expect(window.authapp.listAccounts).toHaveBeenCalledTimes(1);

    await user.keyboard('{Control>}f{/Control}');
    const dialog = screen.getByRole('dialog');
    await waitFor(() => expect(dialog).toHaveAttribute('open'));

    expect(await within(dialog).findByText('482 913')).toBeVisible();
    expect(within(dialog).getByText('731 042')).toBeVisible();
    expect(within(dialog).queryByText(/Kayıtlı hesap yok/)).not.toBeInTheDocument();

    const input = within(dialog).getByPlaceholderText('Hesap veya komut ara');
    await user.type(input, 'Discord');

    expect(within(dialog).getByText('Discord')).toBeVisible();
    expect(within(dialog).getByText('arda@example.com')).toBeVisible();
    expect(within(dialog).queryByText('Instagram')).not.toBeInTheDocument();
  });

  it('matches lowercase "instagram" against "Instagram" in Turkish locale and copies TOTP on click', async () => {
    const user = userEvent.setup();
    renderApp();
    await screen.findByText('Instagram');

    await user.click(screen.getByRole('button', { name: 'Hesaplarda ara' }));
    const dialog = screen.getByRole('dialog');
    const input = await within(dialog).findByPlaceholderText('Hesap veya komut ara');
    await within(dialog).findByText('731 042');

    await user.type(input, 'instagram');
    const instagramButton = within(dialog).getByRole('button', { name: /Instagram.*Kopyala/i });
    expect(within(dialog).queryByText('Discord')).not.toBeInTheDocument();

    await user.click(instagramButton);
    expect(window.authapp.copyTotp).toHaveBeenCalledWith(instagramAccount.id);
    expect(await within(dialog).findByText('Kopyalandı')).toBeVisible();
  });

  it('supports keyboard Enter to copy top matching account and Escape to close', async () => {
    const user = userEvent.setup();
    renderApp();
    await screen.findByText('Discord');

    await user.keyboard('{Control>}f{/Control}');
    const dialog = screen.getByRole('dialog');
    const input = await within(dialog).findByPlaceholderText('Hesap veya komut ara');
    await within(dialog).findByText('482 913');

    await user.type(input, 'arda@example');
    await user.keyboard('{Enter}');
    expect(window.authapp.copyTotp).toHaveBeenCalledWith(discordAccount.id);

    await user.keyboard('{Escape}');
    await waitFor(() => expect(dialog).not.toHaveAttribute('open'));
  });

  it('navigates to Home and highlights the selected account when clicking the jump button', async () => {
    const user = userEvent.setup();
    renderApp(['/settings']);
    expect(await screen.findByRole('heading', { name: 'Ayarlar' })).toBeVisible();

    await user.keyboard('{Control>}f{/Control}');
    const dialog = screen.getByRole('dialog');
    await within(dialog).findByText('482 913');

    await user.click(within(dialog).getByRole('button', { name: 'Discord - Hesaba git' }));
    await waitFor(() => expect(dialog).not.toHaveAttribute('open'));

    const homeHeading = await screen.findByRole('heading', { name: 'Doğrulama Kodları' });
    expect(homeHeading).toBeVisible();

    await waitFor(() => {
      const card = document.querySelector(`[data-account-id="${discordAccount.id}"]`);
      expect(card).toHaveClass('is-highlighted');
    });
  });

  it('keeps broken accounts visible with disabled copy and allows jumping to home', async () => {
    vi.mocked(window.authapp.listAccounts).mockResolvedValue([discordAccount, brokenAccount]);
    vi.mocked(window.authapp.getTotpCodes).mockResolvedValue([
      { accountId: discordAccount.id, code: '482913', remaining: 15, period: 30 },
    ]);

    const user = userEvent.setup();
    renderApp();
    await screen.findByText('GitHub');

    await user.keyboard('{Control>}f{/Control}');
    const dialog = screen.getByRole('dialog');
    await within(dialog).findByText('482 913');

    const githubButton = within(dialog).getByRole('button', { name: /GitHub.*Kod üretilemedi/i });
    expect(githubButton).toBeDisabled();
    expect(within(dialog).getByRole('button', { name: 'GitHub - Hesaba git' })).toBeEnabled();
  });

  it('shows empty-vault commands when no accounts exist and shows no-match message on unmatched query', async () => {
    vi.mocked(window.authapp.listAccounts).mockResolvedValue([]);
    vi.mocked(window.authapp.getTotpCodes).mockResolvedValue([]);

    const user = userEvent.setup();
    renderApp();
    await screen.findByRole('heading', { name: 'Henüz hesap eklenmedi' });

    await user.keyboard('{Control>}f{/Control}');
    const dialog = screen.getByRole('dialog');
    expect(
      await within(dialog).findByText('Kayıtlı hesap yok. Kullanılabilir uygulama komutları:'),
    ).toBeVisible();

    const input = within(dialog).getByPlaceholderText('Hesap veya komut ara');
    await user.type(input, 'olmayan-servis');

    expect(within(dialog).queryByText(/Kayıtlı hesap yok/)).not.toBeInTheDocument();
    expect(within(dialog).getByText('Bu aramayla eşleşen hesap veya komut yok.')).toBeVisible();
  });

  it('shows an error without claiming no accounts exist when listAccounts fails', async () => {
    const user = userEvent.setup();
    renderApp();
    await screen.findByText('Discord');

    vi.mocked(window.authapp.listAccounts).mockRejectedValueOnce(
      new Error('Locked or unavailable'),
    );
    await user.keyboard('{Control>}f{/Control}');

    const dialog = screen.getByRole('dialog');
    expect(await within(dialog).findByText('Hesaplar okunamadı.')).toBeVisible();
    expect(within(dialog).queryByText(/Kayıtlı hesap yok/)).not.toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: 'Ayarları aç' })).toBeVisible();
  });

  it('refreshes TOTP codes on period rollover while search dialog stays open', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    vi.setSystemTime(new Date('2026-11-13T12:00:29.500Z'));

    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    renderApp();
    await screen.findByText('Discord');

    await user.keyboard('{Control>}f{/Control}');
    const dialog = screen.getByRole('dialog');
    expect(await within(dialog).findByText('482 913')).toBeVisible();

    vi.mocked(window.authapp.getTotpCodes).mockResolvedValue([
      { accountId: discordAccount.id, code: '999888', remaining: 30, period: 30 },
      { accountId: instagramAccount.id, code: '111222', remaining: 30, period: 30 },
    ]);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1100);
    });

    expect(await within(dialog).findByText('999 888')).toBeVisible();
  });
});
