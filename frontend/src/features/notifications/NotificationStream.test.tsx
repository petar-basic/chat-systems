import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { globalEventBus } from '@/lib/globalEventBus';
import { useWorkspaceStore } from '@/stores/workspace';
import { NotificationStream } from './NotificationStream';

const navigate = vi.fn();
const playMessageSound = vi.fn();
const showNotification = vi.fn();

vi.mock('react-router', () => ({ useNavigate: () => navigate }));
vi.mock('@/lib/notifications', () => ({
  playMessageSound: () => playMessageSound(),
  showNotification: (...args: unknown[]) => showNotification(...args),
}));

function frame(kind: string, channelId = 'ch-1') {
  return {
    type: 'notification' as const,
    workspace_id: 'ws-1',
    channel_id: channelId,
    message_id: 'm-1' as string | null,
    list_id: null as string | null,
    item_id: null as string | null,
    title: 'Ana in #general',
    body: 'deploy finished',
    kind,
  };
}

describe('NotificationStream', () => {
  beforeEach(() => {
    navigate.mockReset();
    playMessageSound.mockReset();
    showNotification.mockReset();
    vi.spyOn(document, 'hasFocus').mockReturnValue(true);
    useWorkspaceStore.setState({ currentChannel: null, currentConversationId: null });
    render(
      <QueryClientProvider client={new QueryClient()}>
        <NotificationStream />
      </QueryClientProvider>,
    );
  });

  it('rings for a message the server decided to announce', () => {
    globalEventBus.emit(frame('message'));

    expect(playMessageSound).toHaveBeenCalledTimes(1);
    expect(showNotification).toHaveBeenCalledWith('Ana in #general', 'deploy finished', expect.any(Function));
  });

  it('stays quiet for the conversation already on screen', () => {
    useWorkspaceStore.setState({ currentConversationId: 'dm-1' });

    globalEventBus.emit(frame('dm', 'dm-1'));

    expect(playMessageSound).not.toHaveBeenCalled();
    expect(showNotification).not.toHaveBeenCalled();
  });

  it('opens a direct message from its notification', () => {
    globalEventBus.emit(frame('dm', 'dm-1'));

    const onClick = showNotification.mock.calls[0][2] as () => void;
    onClick();
    expect(navigate).toHaveBeenCalledWith('/app/ws-1/c/dm-1');
  });

  it('marks a mention on the channel it came from', () => {
    globalEventBus.emit(frame('mention'));

    expect(useWorkspaceStore.getState().mentionChannels.has('ch-1')).toBe(true);
    expect(showNotification).toHaveBeenCalledWith(
      '🔔 Ana in #general',
      'deploy finished',
      expect.any(Function),
    );
  });

  it('opens a list item from its notification, even from the channel on screen', () => {
    useWorkspaceStore.setState({ currentChannel: { id: 'ch-1' } as never });

    globalEventBus.emit({
      ...frame('list'),
      message_id: null,
      list_id: 'l-1',
      item_id: 'i-1',
      title: 'Ana assigned you to Fix login',
      body: 'in Launch plan',
    });

    expect(playMessageSound).toHaveBeenCalledTimes(1);
    const onClick = showNotification.mock.calls[0][2] as () => void;
    onClick();
    expect(navigate).toHaveBeenCalledWith('/app/ws-1/lists/l-1?item=i-1');
  });
});
