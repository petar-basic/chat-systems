import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import i18n from '@/lib/i18n';
import { useHuddleStore } from '@/stores/huddle';
import { useWorkspaceStore } from '@/stores/workspace';
import { HuddleWindow } from './HuddleWindow';
import type { HuddleControls } from '../HuddleController';

vi.mock('@/features/workspace', () => ({ useWorkspaceMembers: () => ({ data: [] }) }));
const thread = vi.hoisted(() => ({
  root: { id: 'm-root', channel_id: 'ch-1' } as { id: string; channel_id: string } | undefined,
  inChannel: true,
  unread: 0,
}));
vi.mock('@/features/huddle/hooks/useHuddle', () => ({
  useInviteToHuddle: () => ({ mutate: vi.fn() }),
  useHuddleThread: (_id: string, _url: string, enabled: boolean) => ({
    data: enabled && thread.root ? { message: thread.root, in_channel: thread.inChannel } : undefined,
  }),
}));
vi.mock('@/features/threads', () => ({
  useKnownThreadState: (rootId?: string) => ({
    data: rootId ? { root_message_id: rootId, following: true, unread_count: thread.unread } : undefined,
  }),
  ThreadPanel: ({ inChannel, onClose }: { inChannel: boolean; onClose: () => void }) => (
    <div data-qa="thread-panel" data-in-channel={String(inChannel)}>
      <button onClick={onClose}>close thread</button>
    </div>
  ),
}));
vi.mock('../hooks/useMediaDevices', () => ({
  useMediaDevices: () => ({ mics: [], cameras: [], speakers: [] }),
}));

const controls = new Proxy({}, { get: () => vi.fn() }) as unknown as HuddleControls;
const participant = (userId: string) => ({ userId }) as never;

describe('HuddleWindow in Japanese', () => {
  beforeEach(async () => {
    useHuddleStore.setState({
      active: {
        huddleId: 'h-1',
        workspaceId: 'ws-1',
        instanceUrl: 'http://localhost:3000',
        selfUserId: 'u-me',
        scope: { kind: 'channel', channelId: 'ch-1' },
      },
      participants: { 'u-a': participant('u-a'), 'u-b': participant('u-b') },
    });
    useWorkspaceStore.setState({
      currentWorkspace: { id: 'ws-1', instanceUrl: 'http://localhost:3000' } as never,
    });
    await i18n.changeLanguage('ja');
  });
  afterEach(async () => {
    useHuddleStore.setState({ active: null, participants: {} });
    await i18n.changeLanguage('en');
  });

  it('counts the people in the huddle and names the view buttons', () => {
    render(<HuddleWindow controls={controls} />);
    const window = screen.getByRole('dialog', { name: '通話' });
    expect(window.querySelector('span')?.textContent).toBe('通話 ・ 3人');
    expect(screen.getByRole('button', { name: '大きく表示' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '通話を全画面表示' })).toBeInTheDocument();
  });
});

describe('HuddleWindow thread', () => {
  beforeEach(() => {
    thread.root = { id: 'm-root', channel_id: 'ch-1' };
    thread.inChannel = true;
    thread.unread = 0;
    useHuddleStore.setState({
      active: {
        huddleId: 'h-1',
        workspaceId: 'ws-1',
        instanceUrl: 'http://localhost:3000',
        selfUserId: 'u-me',
        scope: { kind: 'channel', channelId: 'ch-1' },
      },
      participants: { 'u-a': participant('u-a') },
      localStream: {} as MediaStream,
    });
    useWorkspaceStore.setState({
      currentWorkspace: { id: 'ws-1', instanceUrl: 'http://localhost:3000' } as never,
    });
  });
  afterEach(() => {
    useHuddleStore.setState({ active: null, participants: {}, localStream: null });
  });

  it('asks for the thread only once the join has gone out', () => {
    useHuddleStore.setState({ localStream: null });
    render(<HuddleWindow controls={controls} />);
    expect(screen.queryByTestId('huddle-thread-toggle')).toBeNull();
  });

  it('counts unread replies while the thread is closed and opens it beside the stage', () => {
    thread.unread = 2;
    render(<HuddleWindow controls={controls} />);
    const toggle = screen.getByRole('button', { name: "Open the huddle's thread, 2 unread replies" });
    expect(screen.getByTestId('huddle-thread-unread')).toHaveTextContent('2');
    expect(screen.queryByTestId('thread-panel')).toBeNull();

    fireEvent.click(toggle);
    expect(screen.getByTestId('thread-panel')).toHaveAttribute('data-in-channel', 'true');
    expect(screen.getByRole('button', { name: "Close the huddle's thread" })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    expect(screen.queryByTestId('huddle-thread-unread')).toBeNull();

    fireEvent.click(screen.getByRole('button', { name: 'close thread' }));
    expect(screen.queryByTestId('thread-panel')).toBeNull();
  });

  it('tells the panel when the reader is only in the huddle', () => {
    thread.inChannel = false;
    render(<HuddleWindow controls={controls} />);
    fireEvent.click(screen.getByRole('button', { name: "Open the huddle's thread" }));
    expect(screen.getByTestId('thread-panel')).toHaveAttribute('data-in-channel', 'false');
  });

  it('offers no thread for a huddle in another workspace', () => {
    useWorkspaceStore.setState({
      currentWorkspace: { id: 'ws-2', instanceUrl: 'http://localhost:3000' } as never,
    });
    render(<HuddleWindow controls={controls} />);
    expect(screen.queryByTestId('huddle-thread-toggle')).toBeNull();
  });
});
