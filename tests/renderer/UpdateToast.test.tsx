import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { UpdateToast } from '../../src/renderer/src/components/UpdateToast';
import { LanguageProvider } from '../../src/renderer/src/app/LanguageProvider';

function renderToast(): void {
  render(
    <LanguageProvider>
      <UpdateToast />
    </LanguageProvider>,
  );
}

describe('UpdateToast', () => {
  it('automatically checks for updates and displays the available version', async () => {
    vi.mocked(window.authapp.checkUpdate).mockResolvedValueOnce({
      hasUpdate: true,
      currentVersion: '0.3.5',
      latestVersion: '0.3.6',
      releaseUrl: 'https://github.com/ardakrt/Authly-Rust/releases/tag/v0.3.6',
    });

    renderToast();

    expect(
      await screen.findByRole('dialog', { name: 'Güncelleme Bulundu' }, { timeout: 4000 }),
    ).toBeInTheDocument();
    expect(screen.getByText('v0.3.6')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'İndir ve Kur' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Daha sonra' })).toBeInTheDocument();
  });

  it('starts the in-app download and install flow', async () => {
    vi.mocked(window.authapp.checkUpdate).mockResolvedValueOnce({
      hasUpdate: true,
      currentVersion: '0.3.5',
      latestVersion: '0.3.6',
      releaseUrl: 'https://github.com/ardakrt/Authly-Rust/releases/tag/v0.3.6',
    });

    const user = userEvent.setup();
    renderToast();

    const installButton = await screen.findByRole(
      'button',
      { name: 'İndir ve Kur' },
      { timeout: 4000 },
    );
    await user.click(installButton);

    expect(window.authapp.installUpdate).toHaveBeenCalledOnce();
    expect(screen.getByRole('dialog', { name: 'Güncelleme Bulundu' })).toBeInTheDocument();
  });

  it('dismisses the toast when later is clicked', async () => {
    vi.mocked(window.authapp.checkUpdate).mockResolvedValueOnce({
      hasUpdate: true,
      currentVersion: '0.3.5',
      latestVersion: '0.3.6',
      releaseUrl: 'https://github.com/ardakrt/Authly-Rust/releases/tag/v0.3.6',
    });

    const user = userEvent.setup();
    renderToast();

    const laterButton = await screen.findByRole(
      'button',
      { name: 'Daha sonra' },
      { timeout: 4000 },
    );
    await user.click(laterButton);

    expect(screen.queryByRole('dialog', { name: 'Güncelleme Bulundu' })).not.toBeInTheDocument();
  });
});
