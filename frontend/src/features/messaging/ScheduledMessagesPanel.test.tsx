import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import i18n from '@/lib/i18n';
import type { Channel } from '@/stores/workspace';
import type { ScheduledMessage } from '@/features/messaging/hooks/useScheduledMessages';
import ScheduledMessagesPanel from './ScheduledMessagesPanel';

let scheduled: ScheduledMessage[] = [];
const idle = { mutate: vi.fn(), mutateAsync: vi.fn(), isPending: false };
vi.mock('@/features/messaging/hooks/useScheduledMessages', () => ({
  useScheduledMessages: () => ({ data: scheduled, isLoading: false }),
  useCancelScheduledMessage: () => idle,
  useRescheduleMessage: () => idle,
  useSendScheduledNow: () => idle,
}));

const hostile = '<img src=x onerror="alert(1)"> {{name}}';
const channels = [{ id: 'ch-1', name: hostile }] as unknown as Channel[];

const message = (over: Partial<ScheduledMessage>): ScheduledMessage =>
  ({
    id: 's-1',
    channel_id: 'ch-1',
    content: 'later',
    attachment_ids: [],
    also_in_channel: false,
    send_at: '2030-01-01T09:00:00Z',
    created_at: '2026-11-10T09:00:00Z',
    thread_parent_id: null,
    failure: null,
    ...over,
  }) as unknown as ScheduledMessage;

function renderPanel() {
  render(
    <ScheduledMessagesPanel workspaceId="ws-1" channels={channels} conversations={[]} onClose={vi.fn()} />,
  );
}

describe('ScheduledMessagesPanel in Japanese', () => {
  afterEach(async () => {
    scheduled = [];
    await i18n.changeLanguage('en');
  });

  it('says nothing is waiting', async () => {
    await i18n.changeLanguage('ja');
    renderPanel();
    expect(
      screen.getByText(
        '送信待ちの項目はありません。メッセージを作成し、送信ボタン横にある時計アイコンを押してください。',
      ),
    ).toBeInTheDocument();
  });

  it('names where each message goes, as text, and why one was not sent', async () => {
    scheduled = [
      message({ id: 's-1', attachment_ids: ['f-1', 'f-2'] }),
      message({ id: 's-2', thread_parent_id: 'm-1', failure: 'channel_archived' }),
      message({ id: 's-3', channel_id: 'gone', failure: 'something_new' }),
    ];
    await i18n.changeLanguage('ja');
    renderPanel();

    const targets = screen.getAllByTestId('scheduled-target').map((el) => el.textContent);
    expect(targets).toEqual([`#${hostile}`, `#${hostile} · スレッド`, '退出したチャンネル']);
    expect(screen.getByTestId('scheduled-files')).toHaveTextContent('ファイル 2件');
    expect(screen.getByRole('button', { name: `#${hostile} への予約の日時を変更` })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: `#${hostile} への予約を取り消す` })).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: `#${hostile} · スレッド へのメッセージを削除` }),
    ).toBeInTheDocument();

    const failures = screen.getAllByTestId('scheduled-failure').map((el) => el.textContent);
    expect(failures).toEqual([
      '送信されませんでした。チャンネルがアーカイブされました。',
      '送信されませんでした。サーバーが配信できませんでした。',
    ]);
    expect(screen.getAllByRole('button', { name: '今すぐ送信' })).toHaveLength(2);
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  it('says why a message to someone who left was not sent', async () => {
    scheduled = [message({ id: 's-1', failure: 'recipient_left' })];
    await i18n.changeLanguage('ja');
    renderPanel();
    expect(screen.getByTestId('scheduled-failure').textContent).toBe(
      '送信されませんでした。送信先の人は、ワークスペースから退出しました。',
    );
  });
});
