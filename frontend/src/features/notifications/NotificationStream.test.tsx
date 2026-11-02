import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { globalEventBus } from '@/lib/globalEventBus';
import { useWorkspaceStore } from '@/stores/workspace';
import { useInstanceStore } from '@/stores/instances';
import { QUERY_KEYS } from '@/shared/constants';
import { NotificationStream } from './NotificationStream';

const navigate = vi.fn();
const playMessageSound = vi.fn();
const showNotification = vi.fn();
const openMessage = vi.fn();
const readOnScreen = vi.fn();

vi.mock('react-router', () => ({ useNavigate: () => navigate }));
vi.mock('@/features/messaging', () => ({ useOpenMessage: () => openMessage }));
vi.mock('@/lib/notifications', () => ({
  playMessageSound: () => playMessageSound(),
  showNotification: (...args: unknown[]) => showNotification(...args),
}));
vi.mock('@/lib/readOnScreen', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/lib/readOnScreen')>()),
  readOnScreen: (...args: unknown[]) => readOnScreen(...args),
}));

let queryClient: QueryClient;

function frame(kind: string, channelId = 'ch-1') {
  return {
    type: 'notification' as const,
    workspace_id: 'ws-1',
    channel_id: channelId,
    message_id: 'm-1' as string | null,
    thread_parent_id: null as string | null,
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
    openMessage.mockReset();
    playMessageSound.mockReset();
    showNotification.mockReset();
    readOnScreen.mockReset();
    vi.spyOn(document, 'hasFocus').mockReturnValue(true);
    vi.spyOn(document, 'visibilityState', 'get').mockReturnValue('visible');
    useWorkspaceStore.setState({
      currentWorkspace: { id: 'ws-1' } as never,
      currentChannel: null,
      currentConversationId: null,
      unreadChannels: new Set(),
      mentionChannels: new Set(),
    });
    queryClient = new QueryClient();
    render(
      <QueryClientProvider client={queryClient}>
        <NotificationStream />
      </QueryClientProvider>,
    );
  });

  it('rings for a message the server decided to announce, tagged like its push so one replaces the other', () => {
    globalEventBus.emit(frame('message'));

    expect(playMessageSound).toHaveBeenCalledTimes(1);
    expect(showNotification).toHaveBeenCalledWith(
      'Ana in #general',
      'deploy finished',
      'ch-1',
      expect.any(Function),
      { place: '', workspaceId: 'ws-1' },
    );
  });

  it('says where a notification comes from and shows a mention by name', () => {
    useInstanceStore.setState({ instances: [{ url: 'https://chat.example' }] as never });
    queryClient.setQueryData(QUERY_KEYS.workspacesList('https://chat.example'), [
      { id: 'ws-1', name: 'Northwind' },
    ]);
    queryClient.setQueryData(QUERY_KEYS.workspaceChannels('ws-1'), [{ id: 'ch-1', name: 'random' }]);

    globalEventBus.emit({ ...frame('mention'), body: '@[Tom Becker](u-1) can you look?' });

    expect(showNotification).toHaveBeenCalledWith(
      '🔔 Ana in #general',
      '@Tom Becker can you look?',
      'ch-1',
      expect.any(Function),
      { place: 'Northwind · #random', workspaceId: 'ws-1' },
    );
  });

  it('stays quiet for the conversation already on screen', () => {
    useWorkspaceStore.setState({ currentConversationId: 'dm-1' });

    globalEventBus.emit(frame('dm', 'dm-1'));

    expect(playMessageSound).not.toHaveBeenCalled();
    expect(showNotification).not.toHaveBeenCalled();
    expect(readOnScreen).toHaveBeenCalledWith(expect.anything(), 'dm-1');
  });

  it('reads a mention in the channel on screen instead of badging it', () => {
    useWorkspaceStore.setState({ currentChannel: { id: 'ch-1' } as never });

    globalEventBus.emit(frame('mention'));

    expect(readOnScreen).toHaveBeenCalledWith(expect.anything(), 'ch-1');
    expect(playMessageSound).not.toHaveBeenCalled();
  });

  it('leaves the open workspace alone when a mention comes from another one', () => {
    globalEventBus.emit({ ...frame('mention', 'ch-elsewhere'), workspace_id: 'ws-2' });

    const { unreadChannels, mentionChannels } = useWorkspaceStore.getState();
    expect(unreadChannels.has('ch-elsewhere')).toBe(false);
    expect(mentionChannels.has('ch-elsewhere')).toBe(false);
  });

  it('keeps a mention in a thread off the channel badge', () => {
    globalEventBus.emit({ ...frame('mention'), thread_parent_id: 'root-1' });

    expect(useWorkspaceStore.getState().mentionChannels.has('ch-1')).toBe(false);
  });

  it('opens the message a notification is about', () => {
    globalEventBus.emit(frame('dm', 'dm-1'));

    const onClick = showNotification.mock.calls[0][3] as () => void;
    onClick();
    expect(openMessage).toHaveBeenCalledWith('m-1', false, undefined);
  });

  it('opens a reply in its thread', () => {
    globalEventBus.emit({ ...frame('reply'), thread_parent_id: 'root-1' });

    const onClick = showNotification.mock.calls[0][3] as () => void;
    onClick();
    expect(openMessage).toHaveBeenCalledWith('m-1', true, undefined);
  });

  it('opens a message from another server on that server', () => {
    globalEventBus.emit({ ...frame('dm', 'dm-1'), workspace_id: 'ws-elsewhere' }, 'https://other.example');

    const onClick = showNotification.mock.calls[0][3] as () => void;
    onClick();
    expect(openMessage).toHaveBeenCalledWith('m-1', false, 'https://other.example');
  });

  it('marks a mention on the channel it came from', () => {
    globalEventBus.emit(frame('mention'));

    expect(useWorkspaceStore.getState().mentionChannels.has('ch-1')).toBe(true);
    expect(showNotification).toHaveBeenCalledWith(
      '🔔 Ana in #general',
      'deploy finished',
      'ch-1',
      expect.any(Function),
      { place: '', workspaceId: 'ws-1' },
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
    const onClick = showNotification.mock.calls[0][3] as () => void;
    onClick();
    expect(navigate).toHaveBeenCalledWith('/app/ws-1/lists/l-1?item=i-1');
  });
});
