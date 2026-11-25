import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import i18n from '@/lib/i18n';
import { useNotificationPrefs } from '@/stores/notificationPrefs';
import NotificationsPanel from './NotificationsPanel';
import type { AppNotification } from './hooks/useNotifications';

const idle = { mutate: vi.fn(), isPending: false };

function row(overrides: Partial<AppNotification>): AppNotification {
  return {
    id: overrides.notification_type ?? 'n',
    workspace_id: 'ws-1',
    user_id: 'u-1',
    notification_type: 'mention',
    body: '',
    data: {},
    channel_id: null,
    message_id: null,
    thread_parent_id: null,
    in_channel: false,
    list_id: null,
    item_id: null,
    is_read: false,
    created_at: new Date().toISOString(),
    ...overrides,
  };
}

const hostile = '<img src=x onerror="alert(1)"> {{name}}';

let rows: AppNotification[] = [];
const english = [
  row({ notification_type: 'dm', data: { sender_name: 'Ana' }, body: 'lunch?' }),
  row({
    notification_type: 'keyword',
    data: { keyword: 'deploy', channel_name: 'ops' },
    body: 'deploy at 5',
  }),
  row({ notification_type: 'call' }),
  row({
    notification_type: 'system',
    data: { notice: 'not_delivered', failure: 'recipient_left' },
  }),
];

rows = english;

vi.mock('react-router', () => ({ useNavigate: () => vi.fn() }));
vi.mock('./hooks/useNotifications', () => ({
  useNotifications: () => ({ data: rows, isLoading: false, isError: false, refetch: vi.fn() }),
  useUnreadNotificationCount: () => ({ data: rows.length }),
  useMarkNotificationsRead: () => idle,
  useMarkAllNotificationsRead: () => idle,
  useDndStatus: () => ({ data: undefined }),
}));

describe('NotificationsPanel', () => {
  it('writes each row from what it is about, in the reader’s words', () => {
    render(<NotificationsPanel workspaceId="ws-1" onClose={vi.fn()} />);

    const titles = screen.getAllByTestId('notification-title').map((el) => el.textContent);
    expect(titles).toEqual([
      'Ana',
      'Matched "deploy" in #ops',
      'Incoming huddle',
      'Scheduled message was not delivered',
    ]);
    expect(screen.getByText('Someone is starting a huddle')).toBeInTheDocument();
    expect(screen.getByText('The person you wrote to has left the workspace.')).toBeInTheDocument();
  });
});

describe('NotificationsPanel in Japanese', () => {
  afterEach(async () => {
    rows = english;
    await i18n.changeLanguage('en');
  });

  it('writes each row in Japanese, with names as text', async () => {
    rows = [
      row({
        notification_type: 'reaction',
        data: { sender_name: hostile, channel_name: 'ops' },
        body: hostile,
      }),
      row({ notification_type: 'call' }),
    ];
    await i18n.changeLanguage('ja');
    render(<NotificationsPanel workspaceId="ws-1" onClose={vi.fn()} />);

    expect(screen.getAllByTestId('notification-title').map((el) => el.textContent)).toEqual([
      `${hostile} さん（#ops）`,
      '通話の着信',
    ]);
    expect(screen.getByText('誰かが通話を始めようとしています')).toBeInTheDocument();
    expect(document.querySelector('[onerror]')).toBeNull();
  });
});

describe('NotificationsPanel header and filters', () => {
  beforeEach(() => {
    useNotificationPrefs.setState({ soundEnabled: true });
  });

  afterEach(async () => {
    rows = english;
    await i18n.changeLanguage('en');
  });

  it('names its header, its buttons and its filters', () => {
    render(<NotificationsPanel workspaceId="ws-1" onClose={vi.fn()} />);
    expect(screen.getByText('Notifications')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Close notifications' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Mark all read' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Pause notifications' })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Mute notification sound' }));
    expect(screen.getByRole('button', { name: 'Unmute notification sound' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'All' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Unread' })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Mentions' }));
    expect(screen.getByText('No notifications')).toBeInTheDocument();
    expect(screen.getByText("You're all caught up!")).toBeInTheDocument();
  });

  it('names them in Japanese', async () => {
    await i18n.changeLanguage('ja');
    render(<NotificationsPanel workspaceId="ws-1" onClose={vi.fn()} />);
    expect(screen.getByText('通知')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '通知を閉じる' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'すべて既読にする' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '通知を一時停止' })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '通知音をミュート' }));
    expect(screen.getByRole('button', { name: '通知音のミュートを解除' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'すべて' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '未読' })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'メンション' }));
    expect(screen.getByText('通知はありません')).toBeInTheDocument();
    expect(screen.getByText('すべて確認済みです')).toBeInTheDocument();
  });

  it('shows the heading as text', () => {
    i18n.addResource('en', 'notifications', 'panel.title', hostile);
    try {
      render(<NotificationsPanel workspaceId="ws-1" onClose={vi.fn()} />);
      expect(screen.getByText(hostile)).toBeInTheDocument();
      expect(screen.getByRole('button', { name: `Close ${hostile.toLowerCase()}` })).toBeInTheDocument();
      expect(document.querySelector('[onerror]')).toBeNull();
    } finally {
      i18n.addResource('en', 'notifications', 'panel.title', 'Notifications');
    }
  });

  it('has Japanese for every one', () => {
    const leaves = (o: object, at = ''): string[] =>
      Object.entries(o).flatMap(([k, v]) =>
        typeof v === 'object' && v ? leaves(v as object, `${at}${k}.`) : [`${at}${k}`],
      );
    const english = leaves(i18n.getResource('en', 'notifications', 'panel'));
    const japanese = leaves(i18n.getResource('ja', 'notifications', 'panel'));
    expect(english.length).toBeGreaterThan(0);
    expect(japanese).toEqual(english);
  });
});
