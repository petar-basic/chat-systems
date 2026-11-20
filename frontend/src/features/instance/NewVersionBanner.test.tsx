import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { act, render, screen } from '@testing-library/react';
import i18n from '@/lib/i18n';
import NewVersionBanner from './NewVersionBanner';

const hostile = '<img src=x onerror="alert(1)"> {{name}}';

const build = vi.hoisted(() => ({ announce: () => {} }));

vi.mock('@/lib/appVersion', () => ({
  runningBuild: 'b1',
  watchForNewBuild: (_running: string, onNew: () => void) => {
    build.announce = onNew;
    return () => {};
  },
}));

function renderBanner() {
  render(<NewVersionBanner />);
  act(() => build.announce());
  return screen.getByRole('status');
}

describe('NewVersionBanner', () => {
  beforeEach(() => {
    build.announce = () => {};
  });

  it('shows nothing until a new build is deployed', () => {
    render(<NewVersionBanner />);
    expect(screen.queryByRole('status')).toBeNull();
  });

  it('offers a reload once a new build is deployed', () => {
    const banner = renderBanner();
    expect(banner.textContent).toBe('A new version is available — reload to pick it up.Reload');
    expect(screen.getByRole('button', { name: 'Reload' })).toBeInTheDocument();
  });

  it('shows a translation as text', () => {
    i18n.addResource('en', 'common', 'newVersion.title', hostile);
    try {
      const banner = renderBanner();
      expect(banner.querySelector('span')?.textContent).toBe(hostile);
      expect(document.querySelector('[onerror]')).toBeNull();
    } finally {
      i18n.addResource('en', 'common', 'newVersion.title', 'A new version is available');
    }
  });

  describe('in Japanese', () => {
    beforeEach(async () => {
      await i18n.changeLanguage('ja');
    });

    afterEach(async () => {
      await i18n.changeLanguage('en');
    });

    it('says a new version is available and offers a reload', () => {
      const banner = renderBanner();
      expect(banner.textContent).toBe('新しいバージョンがあります。再読み込みすると反映されます。再読み込み');
      expect(screen.getByRole('button', { name: '再読み込み' })).toBeInTheDocument();
    });

    it('has Japanese for every one', () => {
      const english = Object.keys(i18n.getResource('en', 'common', 'newVersion'));
      const japanese = Object.keys(i18n.getResource('ja', 'common', 'newVersion'));
      expect(english.length).toBeGreaterThan(0);
      expect(japanese).toEqual(english);
    });
  });
});
