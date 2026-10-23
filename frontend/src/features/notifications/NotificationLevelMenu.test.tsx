import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useRef } from 'react';
import { render, screen, fireEvent } from '@testing-library/react';
import { NotificationLevelButton, NotificationLevelMenu } from './NotificationLevelMenu';

const setLevel = vi.fn();
const setMuted = vi.fn();
let channel: Record<string, unknown> = {};
let muted = false;

vi.mock('@/stores/workspace', () => ({
  useWorkspaceStore: (select: (state: unknown) => unknown) =>
    select({
      currentWorkspace: { id: 'ws-1', instanceUrl: 'http://localhost:3000' },
      mutedChannels: new Set(muted ? ['ch-1'] : []),
    }),
}));
vi.mock('@/hooks/queries/useWorkspaces', () => ({
  useWorkspaceChannels: () => ({ data: [{ id: 'ch-1', ...channel }] }),
}));
vi.mock('@/hooks/queries/useConversations', () => ({
  useConversations: () => ({ data: [{ id: 'dm-1', notify_level: null, effective_notify_level: 'all' }] }),
}));
vi.mock('@/hooks/queries/useChannels', () => ({
  useSetChannelMuted: () => ({ mutate: setMuted }),
}));
vi.mock('@/hooks/queries/useNotifyLevels', () => ({
  useNotifyDefault: () => ({ data: 'mentions' }),
  useSetNotifyLevel: () => ({ mutate: setLevel }),
}));

function Menu({ direct = false, channelId = 'ch-1' }: { direct?: boolean; channelId?: string }) {
  const anchor = useRef<HTMLButtonElement>(null);
  return (
    <>
      <button ref={anchor}>anchor</button>
      <NotificationLevelMenu channelId={channelId} direct={direct} anchorRef={anchor} onClose={vi.fn()} />
    </>
  );
}

describe('NotificationLevelMenu', () => {
  beforeEach(() => {
    setLevel.mockReset();
    setMuted.mockReset();
    channel = { notify_level: null, effective_notify_level: 'mentions' };
    muted = false;
  });

  it('marks the default as chosen, names what it is, and sets a level', () => {
    render(<Menu />);

    const byDefault = screen.getByRole('menuitemradio', { name: 'Default (Mentions)' });
    expect(byDefault).toHaveAttribute('aria-checked', 'true');
    expect(byDefault).toHaveFocus();

    fireEvent.click(screen.getByRole('menuitemradio', { name: 'All new messages' }));
    expect(setLevel).toHaveBeenCalledWith({ channelId: 'ch-1', level: 'all' }, expect.anything());
  });

  it('goes back to the default from a level set on the channel', () => {
    channel = { notify_level: 'nothing', effective_notify_level: 'nothing' };
    render(<Menu />);

    expect(screen.getByRole('menuitemradio', { name: 'Nothing' })).toHaveAttribute('aria-checked', 'true');
    fireEvent.click(screen.getByRole('menuitemradio', { name: 'Default (Mentions)' }));
    expect(setLevel).toHaveBeenCalledWith({ channelId: 'ch-1', level: null }, expect.anything());
  });

  it('mutes and unmutes a channel, but offers no mute for a DM', () => {
    const { unmount } = render(<Menu />);
    fireEvent.click(screen.getByRole('menuitemcheckbox', { name: 'Mute channel' }));
    expect(setMuted).toHaveBeenCalledWith({ channelId: 'ch-1', muted: true });
    unmount();

    render(<Menu direct channelId="dm-1" />);
    expect(screen.getByRole('menuitemradio', { name: 'Default (All new messages)' })).toBeInTheDocument();
    expect(screen.queryByRole('menuitemcheckbox')).toBeNull();
  });

  it('moves between items with the arrow keys, Home and End', () => {
    render(<Menu />);
    const menu = screen.getByRole('menu', { name: 'Notifications' });

    fireEvent.keyDown(menu, { key: 'ArrowDown' });
    expect(screen.getByRole('menuitemradio', { name: 'All new messages' })).toHaveFocus();
    fireEvent.keyDown(menu, { key: 'End' });
    expect(screen.getByRole('menuitemcheckbox', { name: 'Mute channel' })).toHaveFocus();
    fireEvent.keyDown(menu, { key: 'ArrowDown' });
    expect(screen.getByRole('menuitemradio', { name: 'Default (Mentions)' })).toHaveFocus();
  });
});

describe('NotificationLevelButton', () => {
  beforeEach(() => {
    channel = { notify_level: null, effective_notify_level: 'mentions' };
    muted = false;
  });

  it('opens the menu and says whether it is open', () => {
    render(<NotificationLevelButton channelId="ch-1" direct={false} />);
    const button = screen.getByRole('button', { name: 'Notifications' });

    expect(button).toHaveAttribute('aria-expanded', 'false');
    fireEvent.click(button);
    expect(button).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByRole('menu', { name: 'Notifications' })).toBeInTheDocument();
  });
});
