import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import i18n from '@/lib/i18n';
import { TimezonePrompt } from './TimezonePrompt';
import { rememberAsked } from './timezoneQuestion';

const hostile = '<img src=x onerror="alert(1)"> {{name}}';

const mutate = vi.fn();
const device = vi.hoisted(() => ({ zone: 'Asia/Tokyo' }));
let account: { timezone: string | null } | undefined;

vi.mock('./hooks/useProfile', () => ({ useAccount: () => ({ data: account }) }));
vi.mock('./hooks/useTimezone', () => ({ useUpdateTimezone: () => ({ mutate }) }));
vi.mock('@/lib/datetime', () => ({ browserTimezone: () => device.zone }));
vi.mock('@/stores/workspace', () => ({
  useWorkspaceStore: (select: (state: unknown) => unknown) =>
    select({ currentWorkspace: { instanceUrl: 'http://chat.test' } }),
}));

describe('TimezonePrompt', () => {
  beforeEach(() => {
    mutate.mockReset();
    localStorage.clear();
    device.zone = 'Asia/Tokyo';
  });

  it('gives an account with no time zone the one this device has, without asking', () => {
    account = { timezone: null };
    render(<TimezonePrompt />);
    expect(mutate).toHaveBeenCalledWith('Asia/Tokyo');
    expect(screen.queryByTestId('timezone-prompt')).toBeNull();
  });

  it('asks once when the device and the profile differ', () => {
    account = { timezone: 'Europe/Belgrade' };
    const { unmount } = render(<TimezonePrompt />);
    expect(screen.getByTestId('timezone-prompt')).toHaveTextContent('Europe/Belgrade');

    fireEvent.click(screen.getByRole('button', { name: 'Use Asia/Tokyo' }));
    expect(mutate).toHaveBeenCalledWith('Asia/Tokyo');
    unmount();

    render(<TimezonePrompt />);
    expect(screen.queryByTestId('timezone-prompt')).toBeNull();
  });

  it('does not ask again after the person kept the profile’s', () => {
    account = { timezone: 'Europe/Belgrade' };
    const { unmount } = render(<TimezonePrompt />);
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(mutate).not.toHaveBeenCalled();
    unmount();

    render(<TimezonePrompt />);
    expect(screen.queryByTestId('timezone-prompt')).toBeNull();
  });

  it('does not ask about a zone picked by hand in Preferences', () => {
    account = { timezone: 'Asia/Tokyo' };
    const { rerender } = render(<TimezonePrompt />);
    rememberAsked('Asia/Tokyo');
    account = { timezone: 'America/New_York' };
    rerender(<TimezonePrompt />);
    expect(screen.queryByTestId('timezone-prompt')).toBeNull();
  });

  it('says nothing when they agree', () => {
    account = { timezone: 'Asia/Tokyo' };
    render(<TimezonePrompt />);
    expect(screen.queryByTestId('timezone-prompt')).toBeNull();
    expect(mutate).not.toHaveBeenCalled();
  });

  it('says why it asks', () => {
    account = { timezone: 'Europe/Belgrade' };
    render(<TimezonePrompt />);
    expect(screen.getByRole('heading', { name: 'Update your time zone?' })).toBeInTheDocument();
    expect(
      screen.getByText(
        'This device is set to Asia/Tokyo, but your profile says Europe/Belgrade. The local time people see on your profile and your notification schedule follow your profile.',
      ),
    ).toBeInTheDocument();
  });

  it.each([
    ['this device', hostile, 'Europe/Belgrade'],
    ['the profile', 'Asia/Tokyo', hostile],
  ])('shows a zone from %s as text', (_from, onDevice, onProfile) => {
    device.zone = onDevice;
    account = { timezone: onProfile };
    render(<TimezonePrompt />);
    expect(
      screen.getByText(
        `This device is set to ${onDevice}, but your profile says ${onProfile}. The local time people see on your profile and your notification schedule follow your profile.`,
      ),
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: `Use ${onDevice}` })).toBeInTheDocument();
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  describe('in Japanese', () => {
    beforeEach(async () => {
      await i18n.changeLanguage('ja');
    });

    afterEach(async () => {
      await i18n.changeLanguage('en');
    });

    it('asks whether to change the time zone, and changes it', () => {
      account = { timezone: 'Europe/Belgrade' };
      render(<TimezonePrompt />);
      expect(screen.getByRole('heading', { name: 'タイムゾーンを変更しますか？' })).toBeInTheDocument();
      expect(
        screen.getByText(
          'この端末のタイムゾーンは Asia/Tokyo ですが、プロフィールでは Europe/Belgrade になっています。ほかの人があなたのプロフィールで見る現地時刻と、通知を受け取る時間帯は、プロフィールのタイムゾーンに従います。',
        ),
      ).toBeInTheDocument();
      expect(screen.getByRole('button', { name: 'キャンセル' })).toBeInTheDocument();

      fireEvent.click(screen.getByRole('button', { name: 'Asia/Tokyo に変更' }));
      expect(mutate).toHaveBeenCalledWith('Asia/Tokyo');
    });

    it('has Japanese for every one', () => {
      const english = Object.keys(i18n.getResource('en', 'profile', 'region.prompt'));
      const japanese = Object.keys(i18n.getResource('ja', 'profile', 'region.prompt'));
      expect(english.length).toBeGreaterThan(0);
      expect(japanese).toEqual(english);
    });
  });
});
