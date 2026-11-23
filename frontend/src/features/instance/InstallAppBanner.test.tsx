import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import i18n from '@/lib/i18n';
import { usePwaInstall, type InstallOffer } from '@/stores/pwaInstall';
import InstallAppBanner from './InstallAppBanner';

const hostile = '<img src=x onerror="alert(1)"> {{name}}';

const install = vi.fn(async () => {});
const dismiss = vi.fn();

function renderBanner(offer: InstallOffer, dismissed = false) {
  usePwaInstall.setState({ offer, dismissed, install, dismiss });
  return render(<InstallAppBanner />);
}

function banner() {
  return screen.getByTestId('install-banner');
}

describe('InstallAppBanner', () => {
  beforeEach(() => {
    install.mockClear();
    dismiss.mockClear();
  });

  it('shows nothing when there is nothing to offer, or once dismissed', () => {
    const { unmount } = renderBanner('none');
    expect(screen.queryByTestId('install-banner')).toBeNull();
    unmount();

    renderBanner('prompt', true);
    expect(screen.queryByTestId('install-banner')).toBeNull();
  });

  it('offers to install where the browser can', () => {
    renderBanner('prompt');
    expect(banner().textContent).toBe(
      'Install Chat Systems — its own window, and notifications when it is closedInstall',
    );

    fireEvent.click(screen.getByRole('button', { name: 'Install' }));
    expect(install).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole('button', { name: 'Dismiss install prompt' }));
    expect(dismiss).toHaveBeenCalledTimes(1);
  });

  it('tells an iPhone how to add it to the home screen', () => {
    renderBanner('ios');
    expect(banner().textContent).toBe('Install Chat Systems — tap  then “Add to Home Screen”');
    expect(screen.getByLabelText('Share')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Install' })).toBeNull();
  });

  it.each([
    ['installApp.title', `${hostile} — its own window, and notifications when it is closedInstall`],
    ['installApp.window', `Install Chat Systems${hostile}Install`],
  ] as const)('shows %s as text', (key, shown) => {
    const english = i18n.t(key, { ns: 'common' });
    i18n.addResource('en', 'common', key, hostile);
    try {
      renderBanner('prompt');
      expect(banner().textContent).toBe(shown);
      expect(document.querySelector('[onerror]')).toBeNull();
    } finally {
      i18n.addResource('en', 'common', key, english);
    }
  });

  describe('in Japanese', () => {
    beforeEach(async () => {
      await i18n.changeLanguage('ja');
    });

    afterEach(async () => {
      await i18n.changeLanguage('en');
    });

    it('offers to install where the browser can', () => {
      renderBanner('prompt');
      expect(banner().textContent).toBe(
        'Chat Systems をアプリとしてインストールできます。専用のウィンドウで開き、閉じているあいだも通知を受け取れます。インストール',
      );
      expect(screen.getByRole('button', { name: 'インストール' })).toBeInTheDocument();
      expect(screen.getByRole('button', { name: 'インストールの案内を閉じる' })).toBeInTheDocument();
    });

    it('tells an iPhone how to add it to the home screen', () => {
      renderBanner('ios');
      expect(banner().textContent).toBe(
        'Chat Systems をアプリとしてインストールできます。 をタップし、「ホーム画面に追加」を選んでください。',
      );
      expect(screen.getByLabelText('共有')).toBeInTheDocument();
    });

    it('has Japanese for every one', () => {
      const english = Object.keys(i18n.getResource('en', 'common', 'installApp'));
      const japanese = Object.keys(i18n.getResource('ja', 'common', 'installApp'));
      expect(english.length).toBeGreaterThan(0);
      expect(japanese).toEqual(english);
    });
  });
});
