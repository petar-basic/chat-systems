import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { TimezonePrompt } from './TimezonePrompt';
import { rememberAsked } from './timezoneQuestion';

const mutate = vi.fn();
let account: { timezone: string | null } | undefined;

vi.mock('./hooks/useProfile', () => ({ useAccount: () => ({ data: account }) }));
vi.mock('./hooks/useTimezone', () => ({ useUpdateTimezone: () => ({ mutate }) }));
vi.mock('@/lib/datetime', () => ({ browserTimezone: () => 'Asia/Tokyo' }));
vi.mock('@/stores/workspace', () => ({
  useWorkspaceStore: (select: (state: unknown) => unknown) =>
    select({ currentWorkspace: { instanceUrl: 'http://chat.test' } }),
}));

describe('TimezonePrompt', () => {
  beforeEach(() => {
    mutate.mockReset();
    localStorage.clear();
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
});
