import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
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

const rows = [
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
