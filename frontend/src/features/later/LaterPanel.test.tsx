import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, within } from '@testing-library/react';
import i18n from '@/lib/i18n';
import en from '@/locales/en/messages.json';
import ja from '@/locales/ja/messages.json';
import { useUserCache } from '@/stores/users';
import LaterPanel from './LaterPanel';

const routerNavigate = vi.fn();
vi.mock('react-router', () => ({ useNavigate: () => routerNavigate }));

const update = vi.fn();
const remove = vi.fn();
const remind = vi.fn();
const items: Record<string, unknown[]> = {};
vi.mock('@/features/later/hooks/useLater', () => ({
  useLater: (_ws: string, status: string) => ({ data: items[status] ?? [], isLoading: false }),
  useLaterActions: () => ({
    update: { mutate: update },
    remove: { mutate: remove },
    remind: { mutate: remind },
  }),
}));

const base = {
  workspace_id: 'ws',
  created_by: 'me',
  channel_id: 'ch',
  note: null,
  reminded_at: null,
  completed_at: null,
  created_at: '2026-10-22T10:00:00Z',
};

function renderPanel(onOpenMessage = vi.fn(), channelName = 'general') {
  render(
    <LaterPanel
      workspaceId="ws"
      channels={[{ id: 'ch', name: channelName } as never]}
      conversations={[]}
      onClose={vi.fn()}
      onOpenMessage={onOpenMessage}
    />,
  );
  return onOpenMessage;
}

const reset = () => {
  update.mockClear();
  remove.mockClear();
  remind.mockClear();
  routerNavigate.mockClear();
  items.in_progress = [
    {
      ...base,
      id: 'saved',
      status: 'in_progress',
      text: null,
      remind_at: '2000-01-01T09:00:00Z',
      message: {
        id: 'm-1',
        channel_id: 'ch',
        author_id: 'ana',
        content: 'the runbook link',
        attachments: [],
        sent_at: base.created_at,
      },
      list_item: null,
    },
    {
      ...base,
      id: 'rem',
      status: 'in_progress',
      text: 'stand-up',
      remind_at: '2999-01-01T09:00:00Z',
      message: null,
      list_item: null,
    },
    {
      ...base,
      id: 'due',
      status: 'in_progress',
      text: 'Fix login',
      remind_at: '2999-01-02T09:00:00Z',
      message: null,
      list_item: { list_id: 'l-1', item_id: 'i-1', list_title: 'Launch plan' },
    },
  ];
  items.completed = [];
  items.archived = [];
};

describe('LaterPanel', () => {
  beforeEach(reset);

  it('marks what is overdue and what is still to come', () => {
    renderPanel();
    const [saved, reminder] = screen.getAllByTestId('later-row');
    expect(within(saved).getByTestId('later-due')).toHaveTextContent('Overdue');
    expect(within(reminder).getByTestId('later-due')).toHaveTextContent('Due');
    expect(reminder).toHaveTextContent('stand-up');
    expect(screen.getByTestId('later-tab-in_progress')).toHaveTextContent('In progress 3');
  });

  it('ticks an item off, archives it, and opens its message', () => {
    const navigate = renderPanel();
    const [saved] = screen.getAllByTestId('later-row');
    fireEvent.click(within(saved).getByTestId('later-complete'));
    expect(update).toHaveBeenCalledWith({ id: 'saved', status: 'completed' });
    fireEvent.click(within(saved).getByTestId('later-archive'));
    expect(update).toHaveBeenCalledWith({ id: 'saved', status: 'archived' });
    fireEvent.click(within(saved).getByTestId('later-content'));
    expect(navigate).toHaveBeenCalledWith('m-1');
  });

  it('opens a list item due tomorrow on its list', () => {
    renderPanel();
    const due = screen.getAllByTestId('later-row')[2];
    expect(due).toHaveTextContent('Launch plan');
    expect(due).toHaveTextContent('Fix login');
    fireEvent.click(within(due).getByTestId('later-content'));
    expect(routerNavigate).toHaveBeenCalledWith('/app/ws/lists/l-1?item=i-1');
  });

  it('shows the empty state of each tab', () => {
    renderPanel();
    fireEvent.click(screen.getByTestId('later-tab-completed'));
    expect(screen.getByTestId('later-empty')).toHaveTextContent('Nothing completed yet');
  });
});

const reminderFrom = (by: string, text: string, channelId = 'ch') => ({
  ...base,
  id: 'from',
  created_by: by,
  channel_id: channelId,
  status: 'in_progress',
  text,
  remind_at: '2999-01-01T09:00:00Z',
  message: null,
  list_item: null,
});

const leaves = (o: object): [string, string][] =>
  Object.entries(o).flatMap(([k, v]) =>
    typeof v === 'string'
      ? [[k, v] as [string, string]]
      : leaves(v).map(([kk, vv]) => [`${k}.${kk}`, vv] as [string, string]),
  );

describe('LaterPanel in English', () => {
  beforeEach(reset);

  it('keeps the English names and text', () => {
    useUserCache
      .getState()
      .populateUsers([{ id: 'boss', email: 'b@example.com', display_name: 'Bea', avatar_url: null }]);
    items.in_progress = [reminderFrom('boss', 'update the doc', 'gone')];
    renderPanel();
    expect(screen.getByRole('button', { name: 'Close later' })).toBeInTheDocument();
    expect(screen.getByTestId('later-tab-archived').textContent).toBe('Archived');
    const row = screen.getByTestId('later-row');
    expect(within(row).getByTestId('later-due').textContent).toMatch(/^Due /);
    expect(row).toHaveTextContent('a channel you left');
    expect(row).toHaveTextContent('· from Bea');
    expect(within(row).getByRole('checkbox', { name: 'Mark as done' })).toBeInTheDocument();
    expect(within(row).getByRole('button', { name: 'Remind me' })).toBeInTheDocument();
    expect(within(row).getByRole('button', { name: 'Archive' })).toBeInTheDocument();
    expect(within(row).getByRole('button', { name: 'Remove' })).toBeInTheDocument();
    expect(screen.getByPlaceholderText('Remind me to…')).toBeInTheDocument();
  });
});

describe('LaterPanel in Japanese', () => {
  beforeEach(async () => {
    reset();
    await i18n.changeLanguage('ja');
  });

  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('names the panel, its tabs and the close button', () => {
    renderPanel();
    expect(screen.getByRole('heading', { name: '後で' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '「後で」を閉じる' })).toBeInTheDocument();
    expect(screen.getByTestId('later-tab-in_progress').textContent).toBe('進行中 3');
    expect(screen.getByTestId('later-tab-archived').textContent).toBe('アーカイブ');
    expect(screen.getByTestId('later-tab-completed').textContent).toBe('完了');
  });

  it('says when each tab is empty', () => {
    items.in_progress = [];
    renderPanel();
    expect(screen.getByTestId('later-empty').textContent).toBe(
      'メッセージを保存するかリマインダーを設定すると、完了になるまでここに表示されます。',
    );
    fireEvent.click(screen.getByTestId('later-tab-archived'));
    expect(screen.getByTestId('later-empty').textContent).toBe('アーカイブしたものはありません。');
    fireEvent.click(screen.getByTestId('later-tab-completed'));
    expect(screen.getByTestId('later-empty').textContent).toBe('完了したものはまだありません。');
  });

  it('says when each reminder is due', () => {
    renderPanel();
    const [saved, reminder] = screen.getAllByTestId('later-row');
    expect(within(saved).getByTestId('later-due').textContent).toMatch(/^期限切れ · \S/);
    expect(within(reminder).getByTestId('later-due').textContent).toMatch(/\S にリマインド$/);
  });

  it('names the actions on a row', () => {
    items.archived = [{ ...(items.in_progress[1] as object), status: 'archived' }];
    items.completed = [{ ...(items.in_progress[1] as object), status: 'completed' }];
    renderPanel();
    const saved = screen.getAllByTestId('later-row')[0];
    expect(within(saved).getByRole('checkbox', { name: '完了にする' })).toBeInTheDocument();
    expect(within(saved).getByRole('button', { name: 'アーカイブ' })).toBeInTheDocument();
    expect(within(saved).getByRole('button', { name: '「後で」から外す' })).toBeInTheDocument();
    fireEvent.click(screen.getByTestId('later-tab-archived'));
    expect(
      within(screen.getByTestId('later-row')).getByRole('button', { name: '進行中に戻す' }),
    ).toBeInTheDocument();
    fireEvent.click(screen.getByTestId('later-tab-completed'));
    expect(
      within(screen.getByTestId('later-row')).getByRole('checkbox', { name: '未完了に戻す' }),
    ).toBeInTheDocument();
  });

  it('names who set a reminder, and a channel the person left', () => {
    useUserCache
      .getState()
      .populateUsers([{ id: 'boss', email: 'b@example.com', display_name: 'Bea', avatar_url: null }]);
    items.in_progress = [reminderFrom('boss', 'update the doc', 'gone')];
    renderPanel();
    const row = screen.getByTestId('later-row');
    expect(within(row).getByText('退出したチャンネル')).toBeInTheDocument();
    expect(within(row).getByText('· Bea から')).toBeInTheDocument();
  });

  it('adds a reminder and picks its time in Japanese', () => {
    renderPanel();
    const input = screen.getByLabelText('新しいリマインダー');
    expect(input).toHaveAttribute('placeholder', 'リマインドする内容…');
    fireEvent.change(input, { target: { value: 'call back' } });
    fireEvent.click(screen.getByRole('button', { name: '日時を選ぶ' }));
    expect(screen.getByLabelText('リマインドする日時')).toBeInTheDocument();
    expect(screen.getByText('この日時に設定').closest('button')).toBeDisabled();
    expect(screen.queryByText('リマインダーを解除')).toBeNull();
  });

  it('offers to clear a reminder that has one', () => {
    renderPanel();
    const reminder = screen.getAllByTestId('later-row')[1];
    fireEvent.click(within(reminder).getByRole('button', { name: 'リマインド' }));
    fireEvent.click(screen.getByText('リマインダーを解除'));
    expect(update).toHaveBeenCalledWith({ id: 'rem', clearReminder: true });
  });

  it('has Japanese for every string', () => {
    const jaLeaves = leaves(ja.later);
    expect(jaLeaves.map(([k]) => k).sort()).toEqual(
      leaves(en.later)
        .map(([k]) => k)
        .sort(),
    );
    for (const [key, value] of jaLeaves) expect(value, key).toMatch(/[\u3040-\u30ff\u4e00-\u9fff]/);
  });
});

describe.each(['en', 'ja'])('LaterPanel with hostile text (%s)', (language) => {
  const hostile = '<img src=x onerror="alert(1)"> {{name}}';

  beforeEach(async () => {
    reset();
    await i18n.changeLanguage(language);
  });

  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('shows a reminder, its setter and its channel as text', () => {
    useUserCache
      .getState()
      .populateUsers([{ id: 'boss', email: 'b@example.com', display_name: hostile, avatar_url: null }]);
    items.in_progress = [{ ...reminderFrom('boss', hostile), note: hostile }];
    renderPanel(vi.fn(), hostile);
    const row = screen.getByTestId('later-row');
    expect(within(row).getByTestId('later-content').textContent).toBe(hostile);
    expect(within(row).getByText(`#${hostile}`).textContent).toBe(`#${hostile}`);
    const from = language === 'ja' ? `· ${hostile} から` : `· from ${hostile}`;
    expect(within(row).getByText(from).textContent).toBe(from);
    expect(
      within(row)
        .getAllByText(hostile)
        .map((e) => e.textContent),
    ).toEqual([hostile, hostile]);
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  it('shows a list title as text', () => {
    items.in_progress = [
      { ...reminderFrom('me', 'Fix login'), list_item: { list_id: 'l', item_id: 'i', list_title: hostile } },
    ];
    renderPanel();
    expect(within(screen.getByTestId('later-row')).getByText(hostile).textContent).toBe(hostile);
    expect(document.querySelector('[onerror]')).toBeNull();
  });
});
