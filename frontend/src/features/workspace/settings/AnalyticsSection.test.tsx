import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { ReactNode } from 'react';
import { render, screen, fireEvent, waitFor, within } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { WorkspaceMember } from '@/stores/workspace';
import { downloadCsv } from '@/lib/csv';
import { AnalyticsSection } from './AnalyticsSection';

const get = vi.fn();
const post = vi.fn();

vi.mock('@/shared/hooks/useCurrentApi', () => ({
  getApiForInstance: () => ({
    typed: async (call: (c: { GET: typeof get; POST: typeof post }) => Promise<{ data: unknown }>) =>
      (await call({ GET: get, POST: post })).data,
  }),
}));

vi.mock('@/shared/components/Toast', () => ({ toast: { success: vi.fn(), error: vi.fn() } }));

vi.mock('@/lib/csv', async (original) => ({
  ...(await original<typeof import('@/lib/csv')>()),
  downloadCsv: vi.fn(),
}));

const DAYS = ['2026-11-01', '2026-11-02', '2026-11-03'];

function day(date: string, values: Partial<Record<string, number>> = {}) {
  return {
    day: date,
    members: 3,
    active_members: 0,
    joined: 0,
    public_messages: 0,
    private_messages: 0,
    direct_messages: 0,
    files: 0,
    reactions: 0,
    ...values,
  };
}

const quiet = {
  from: DAYS[0],
  to: DAYS[2],
  members: 3,
  active_members: 0,
  daily: DAYS.map((d) => day(d)),
  weekly: [],
};

const busy = {
  ...quiet,
  active_members: 2,
  daily: [
    day(DAYS[0], { active_members: 2, joined: 1, public_messages: 4, direct_messages: 1, reactions: 2 }),
    day(DAYS[1], { active_members: 1, private_messages: 3, files: 1 }),
    day(DAYS[2]),
  ],
  weekly: [{ week: '2026-10-26', active_members: 2 }],
};

const activity = [
  { user_id: 'u-ana', role: 'member', days_active: 3, messages: 10, last_active: DAYS[1] },
  { user_id: 'u-bo', role: 'admin', days_active: 5, messages: 2, last_active: DAYS[0] },
  { user_id: 'u-cy', role: 'guest', days_active: 0, messages: 0, last_active: null },
];

const members = [
  { user_id: 'u-ana', display_name: 'Ana', email: 'ana@example.com' },
  { user_id: 'u-bo', display_name: 'Bo', email: 'bo@example.com' },
  { user_id: 'u-cy', display_name: null, email: 'cy@example.com' },
] as WorkspaceMember[];

function channel(id: string, name: string, values: object = {}) {
  return {
    channel_id: id,
    name,
    channel_type: 'public',
    is_archived: false,
    created_at: '2026-01-15T10:00:00Z',
    members: 3,
    guests: 0,
    messages: 0,
    posters: 0,
    viewers: 0,
    reactions: 0,
    last_message_at: null,
    ...values,
  };
}

const channels = [
  channel('c-general', 'general', {
    messages: 4,
    posters: 2,
    viewers: 3,
    guests: 1,
    reactions: 5,
    last_message_at: '2026-11-03T09:00:00Z',
  }),
  channel('c-secret', 'secret', { channel_type: 'private', messages: 9, posters: 1, viewers: 1 }),
  channel('c-old', 'old', { is_archived: true }),
];

const quietChannels = [
  {
    channel_id: 'c-ghost',
    name: 'ghost',
    channel_type: 'public',
    members: 1,
    last_message_at: null,
    created_at: '2026-01-01T00:00:00Z',
  },
  {
    channel_id: 'c-attic',
    name: 'attic',
    channel_type: 'private',
    members: 4,
    last_message_at: '2026-05-02T10:00:00Z',
    created_at: '2025-01-01T00:00:00Z',
  },
];

function answer(report: object) {
  get.mockImplementation(async (path: string) => {
    if (path.endsWith('/members')) return { data: { data: activity } };
    if (path.endsWith('/channels')) return { data: { data: channels } };
    if (path.endsWith('/quiet-channels')) return { data: { data: quietChannels } };
    return { data: report };
  });
}

function wrapper({ children }: { children: ReactNode }) {
  return (
    <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
      {children}
    </QueryClientProvider>
  );
}

function renderSection() {
  render(<AnalyticsSection workspaceId="ws-1" workspaceName="Dev Team" members={members} />, { wrapper });
}

const names = () =>
  screen.queryAllByTestId('analytics-member-row').map((row) => row.querySelector('td')?.textContent);

const channelNames = () =>
  screen.queryAllByTestId('analytics-channel-row').map((row) => row.querySelector('td')?.textContent);

describe('AnalyticsSection', () => {
  beforeEach(() => {
    get.mockReset();
    post.mockReset();
    vi.mocked(downloadCsv).mockReset();
  });

  it('says there is nothing yet when the days are empty', async () => {
    answer(quiet);
    renderSection();

    expect(await screen.findAllByText('Nothing in these days yet.')).toHaveLength(2);
    expect(screen.queryByRole('img')).not.toBeInTheDocument();
  });

  it('draws the active members and the messages of each day', async () => {
    answer(busy);
    renderSection();

    expect(
      await screen.findByRole('img', { name: 'Messages per day: at most 5 on a day' }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole('img', { name: 'Active members per day: at most 2 on a day' }),
    ).toBeInTheDocument();
    expect(screen.getByText('Week of Oct 26:')).toBeInTheDocument();
    const messages = screen.getByText('Messages', { selector: 'div' }).parentElement!;
    expect(within(messages).getByText('8')).toBeInTheDocument();
  });

  it('asks again for another period, without guests', async () => {
    answer(busy);
    renderSection();
    await screen.findByText('Members', { selector: 'h3' });

    fireEvent.change(screen.getByLabelText('Period'), { target: { value: '90' } });
    fireEvent.click(screen.getByText('Include guests'));

    await waitFor(() =>
      expect(get).toHaveBeenCalledWith('/workspaces/{ws_id}/analytics', {
        params: { path: { ws_id: 'ws-1' }, query: { days: 90, guests: false } },
      }),
    );
  });

  it('sorts the member table by the column clicked', async () => {
    answer(busy);
    renderSection();
    await waitFor(() => expect(names()).toEqual(['Bo', 'Ana', 'cy@example.com']));

    fireEvent.click(screen.getByRole('button', { name: 'Messages' }));
    expect(names()).toEqual(['Ana', 'Bo', 'cy@example.com']);
    expect(screen.getByRole('columnheader', { name: 'Messages' })).toHaveAttribute('aria-sort', 'descending');

    fireEvent.click(screen.getByRole('button', { name: 'Messages' }));
    expect(names()).toEqual(['cy@example.com', 'Bo', 'Ana']);

    fireEvent.click(screen.getByRole('button', { name: 'Member' }));
    expect(names()).toEqual(['Ana', 'Bo', 'cy@example.com']);
    expect(screen.getByRole('columnheader', { name: 'Member' })).toHaveAttribute('aria-sort', 'ascending');
  });

  it('exports each table with the rows it shows', async () => {
    answer(busy);
    renderSection();
    await waitFor(() => expect(names()).toHaveLength(3));

    fireEvent.click(screen.getByTestId('analytics-members-csv'));
    expect(downloadCsv).toHaveBeenCalledWith('dev-team-members-30-days.csv', [
      ['Member', 'Role', 'Days active', 'Messages', 'Last active'],
      ['Ana', 'member', 3, 10, DAYS[1]],
      ['Bo', 'admin', 5, 2, DAYS[0]],
      ['cy@example.com', 'guest', 0, 0, null],
    ]);

    fireEvent.click(screen.getByTestId('analytics-days-csv'));
    const [file, rows] = vi.mocked(downloadCsv).mock.calls[1];
    expect(file).toBe('dev-team-days-30-days.csv');
    expect(rows).toHaveLength(4);
    expect(rows[1]).toEqual([DAYS[0], 3, 2, 1, 4, 0, 1, 0, 2]);
  });

  it('lists the channels busiest first and exports them', async () => {
    answer(busy);
    renderSection();
    fireEvent.click(screen.getByRole('tab', { name: 'Channels' }));

    await waitFor(() => expect(channelNames()).toEqual(['secret', 'general', 'old']));
    fireEvent.click(screen.getByRole('button', { name: 'Viewed' }));
    expect(channelNames()).toEqual(['general', 'secret', 'old']);
    expect(screen.getByLabelText('Archived')).toBeInTheDocument();
    expect(screen.getByLabelText('Private')).toBeInTheDocument();

    fireEvent.click(screen.getByTestId('analytics-channels-csv'));
    expect(downloadCsv).toHaveBeenCalledWith('dev-team-channels-30-days.csv', [
      [
        'Channel',
        'Type',
        'Archived',
        'Created',
        'Members',
        'Guests',
        'Messages',
        'Members who posted',
        'Members who viewed',
        'Reactions',
        'Last message',
      ],
      ['general', 'public', 'no', '2026-01-15', 3, 1, 4, 2, 3, 5, '2026-11-03'],
      ['secret', 'private', 'no', '2026-01-15', 3, 0, 9, 1, 1, 0, null],
      ['old', 'public', 'yes', '2026-01-15', 3, 0, 0, 0, 0, 0, null],
    ]);
  });

  it('drops the guests column when guests are left out', async () => {
    answer(busy);
    renderSection();
    fireEvent.click(screen.getByRole('tab', { name: 'Channels' }));
    expect(await screen.findByRole('button', { name: 'Guests' })).toBeInTheDocument();

    fireEvent.click(screen.getByText('Include guests'));

    await waitFor(() => expect(screen.queryByRole('button', { name: 'Guests' })).not.toBeInTheDocument());
    expect(get).toHaveBeenCalledWith('/workspaces/{ws_id}/analytics/channels', {
      params: { path: { ws_id: 'ws-1' }, query: { days: 30, guests: false } },
    });
  });

  it('archives the quiet channels picked, after asking once', async () => {
    answer(busy);
    post.mockResolvedValue({ data: { archived: ['c-ghost', 'c-attic'] } });
    renderSection();
    fireEvent.click(screen.getByRole('tab', { name: 'Quiet channels' }));

    expect(await screen.findAllByTestId('analytics-quiet-row')).toHaveLength(2);
    expect(get).toHaveBeenCalledWith('/workspaces/{ws_id}/analytics/quiet-channels', {
      params: { path: { ws_id: 'ws-1' }, query: { days: 90 } },
    });
    expect(screen.getByTestId('analytics-quiet-archive-selected')).toBeDisabled();

    fireEvent.click(screen.getByText('ghost'));
    fireEvent.click(screen.getByText('attic'));
    fireEvent.click(screen.getByRole('button', { name: 'Archive 2 channels' }));

    const dialog = await screen.findByTestId('analytics-quiet-confirm');
    expect(within(dialog).getByText('#ghost')).toBeInTheDocument();
    expect(within(dialog).getByText('#attic')).toBeInTheDocument();
    expect(post).not.toHaveBeenCalled();

    fireEvent.click(screen.getByTestId('analytics-quiet-confirm-confirm'));
    await waitFor(() =>
      expect(post).toHaveBeenCalledWith('/workspaces/{ws_id}/analytics/quiet-channels/archive', {
        params: { path: { ws_id: 'ws-1' } },
        body: { channel_ids: ['c-ghost', 'c-attic'] },
      }),
    );
  });

  it('archives one quiet channel from its own row', async () => {
    answer(busy);
    post.mockResolvedValue({ data: { archived: ['c-attic'] } });
    renderSection();
    fireEvent.click(screen.getByRole('tab', { name: 'Quiet channels' }));

    fireEvent.click(await screen.findByRole('button', { name: 'Archive #attic' }));
    expect(await screen.findByText('Archive #attic?')).toBeInTheDocument();
    fireEvent.click(screen.getByTestId('analytics-quiet-confirm-confirm'));

    await waitFor(() =>
      expect(post).toHaveBeenCalledWith('/workspaces/{ws_id}/analytics/quiet-channels/archive', {
        params: { path: { ws_id: 'ws-1' } },
        body: { channel_ids: ['c-attic'] },
      }),
    );
  });

  it('keeps the dialog open with the reason when archiving fails', async () => {
    answer(busy);
    post.mockRejectedValue(new Error('A default channel cannot be archived'));
    renderSection();
    fireEvent.click(screen.getByRole('tab', { name: 'Quiet channels' }));

    fireEvent.click(await screen.findByRole('button', { name: 'Archive #ghost' }));
    fireEvent.click(await screen.findByTestId('analytics-quiet-confirm-confirm'));

    expect(await screen.findByText('A default channel cannot be archived')).toBeInTheDocument();
    expect(screen.getByTestId('analytics-quiet-confirm')).toBeInTheDocument();
  });
});
