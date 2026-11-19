import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import i18n from '@/lib/i18n';
import { useUserCache } from '@/stores/users';
import { useWorkspaceStore, type Message } from '@/stores/workspace';
import { useHuddleStore } from '@/stores/huddle';
import { HuddleSystemMessage } from './HuddleSystemMessage';

const hostile = '<img src=x onerror="alert(1)"> {{name}}';

const started = (over: Partial<Message> = {}, metadata: Message['metadata'] = {}): Message =>
  ({
    id: 'm-1',
    channel_id: 'ch-1',
    user_id: 'u-x',
    content: 'started a huddle',
    created_at: '2026-11-16T10:00:00Z',
    reply_count: 0,
    metadata: { kind: 'huddle_started', huddle_id: 'h-1', initiator_id: 'u-x', ...metadata },
    ...over,
  }) as Message;

const people = (...names: string[]) =>
  new Map(names.map((name, i) => [`u-${i}`, { id: `u-${i}`, display_name: name } as never]));

describe('HuddleSystemMessage in Japanese', () => {
  beforeEach(async () => {
    useUserCache.setState({ users: new Map([['u-x', { id: 'u-x', display_name: hostile } as never]]) });
    useWorkspaceStore.setState({
      currentWorkspace: { id: 'ws-1', instanceUrl: 'http://localhost:3000' } as never,
      currentUserId: 'u-me',
      activeHuddleChannels: new Map([['ch-1', { huddleId: 'h-1' }]]) as never,
    });
    useHuddleStore.setState({ active: null });
    await i18n.changeLanguage('ja');
  });
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('says who started the huddle, as text, and offers to join', () => {
    const { container } = render(<HuddleSystemMessage message={started()} />);
    expect(container.textContent).toBe(`${hostile} さんが通話を開始しました参加`);
    expect(screen.getByText(hostile)).toHaveClass('font-medium');
    expect(screen.getByRole('button', { name: '参加' })).toBeInTheDocument();
    expect(document.querySelector('[onerror]')).toBeNull();
  });
});

describe('HuddleSystemMessage once the huddle ended', () => {
  beforeEach(() => {
    useUserCache.setState({ users: people('Ana', 'Bo', 'Cy', 'Di', 'Ed') });
    useWorkspaceStore.setState({
      currentWorkspace: { id: 'ws-1', instanceUrl: 'http://localhost:3000' } as never,
      currentUserId: 'u-me',
      activeHuddleChannels: new Map([['ch-1', { huddleId: 'h-1' }]]) as never,
    });
    useHuddleStore.setState({ active: null });
  });

  it('says how long it lasted and who was in it, and no longer offers to join', () => {
    render(
      <HuddleSystemMessage
        message={started(
          { user_id: 'u-0' },
          { initiator_id: 'u-0', ended_at: '2026-11-16T10:12:20Z', participant_ids: ['u-0', 'u-1'] },
        )}
      />,
    );
    expect(screen.getByText('Lasted 12 minutes · with Ana and Bo')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Join' })).toBeNull();
  });

  it('names three people and counts the rest', () => {
    render(
      <HuddleSystemMessage
        message={started(
          { user_id: 'u-0' },
          {
            initiator_id: 'u-0',
            ended_at: '2026-11-16T11:05:00Z',
            participant_ids: ['u-0', 'u-1', 'u-2', 'u-3', 'u-4'],
          },
        )}
      />,
    );
    expect(screen.getByText('Lasted 1 h 5 min · with Ana, Bo, Cy, and 2 others')).toBeInTheDocument();
  });

  it('calls a short huddle less than a minute', () => {
    render(
      <HuddleSystemMessage
        message={started({ user_id: 'u-0' }, { initiator_id: 'u-0', ended_at: '2026-11-16T10:00:20Z' })}
      />,
    );
    expect(screen.getByTestId('huddle-summary')).toHaveTextContent(/^Lasted less than a minute$/);
  });

  it('opens its thread from the replies link', () => {
    const onThreadOpen = vi.fn();
    const message = started({ user_id: 'u-0', reply_count: 3 }, { ended_at: '2026-11-16T10:30:00Z' });
    render(<HuddleSystemMessage message={message} onThreadOpen={onThreadOpen} />);
    fireEvent.click(screen.getByRole('button', { name: /3 replies/ }));
    expect(onThreadOpen).toHaveBeenCalledWith(message);
  });
});

describe('HuddleSystemMessage once the huddle ended, in Japanese', () => {
  beforeEach(async () => {
    useUserCache.setState({ users: people(hostile, 'Bo', 'Cy', 'Di', 'Ed') });
    useWorkspaceStore.setState({
      currentWorkspace: { id: 'ws-1', instanceUrl: 'http://localhost:3000' } as never,
      currentUserId: 'u-me',
      activeHuddleChannels: new Map() as never,
    });
    useHuddleStore.setState({ active: null });
    await i18n.changeLanguage('ja');
  });
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  const ended = (endedAt: string, participantIds: string[] = []) =>
    started({ user_id: 'u-1' }, { initiator_id: 'u-1', ended_at: endedAt, participant_ids: participantIds });

  it('names three people as text and counts the rest', () => {
    render(
      <HuddleSystemMessage message={ended('2026-11-16T11:05:00Z', ['u-0', 'u-1', 'u-2', 'u-3', 'u-4'])} />,
    );
    expect(screen.getByTestId('huddle-summary').textContent).toBe(
      `通話時間: 1 時間 5 分 · 参加者: ${hostile}、Bo、Cy、ほか 2 人`,
    );
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  it('counts minutes, and calls a short huddle less than a minute', () => {
    const { unmount } = render(<HuddleSystemMessage message={ended('2026-11-16T10:12:20Z', ['u-1'])} />);
    expect(screen.getByTestId('huddle-summary').textContent).toBe('通話時間: 12 分 · 参加者: Bo');
    unmount();

    render(<HuddleSystemMessage message={ended('2026-11-16T10:00:20Z')} />);
    expect(screen.getByTestId('huddle-summary').textContent).toBe('通話時間: 1 分未満');
  });
});
