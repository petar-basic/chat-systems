import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import i18n from '@/lib/i18n';
import type { Message } from '@/stores/workspace';
import { MessageActionsBar } from './MessageActionsBar';
import type { MessageActionHandlers } from './messageActionList';

vi.mock('./EmojiPicker', () => ({ default: () => null }));

const message = { id: 'm-1', is_pinned: false, pending: false } as unknown as Message;

const actions: MessageActionHandlers = {
  saved: false,
  canDelete: true,
  canEdit: true,
  canReact: true,
  onThread: vi.fn(),
  onToggleSave: vi.fn(),
  onRemind: vi.fn(),
  onCopyLink: vi.fn(),
  onMarkUnread: vi.fn(),
  onForward: vi.fn(),
  onTogglePin: vi.fn(),
  onEdit: vi.fn(),
  onDelete: vi.fn(),
};

describe('MessageActionsBar in Japanese', () => {
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('names the quick actions and everything in the more menu', async () => {
    await i18n.changeLanguage('ja');
    render(
      <MessageActionsBar
        message={message}
        actions={actions}
        pickerOpen={false}
        onPickerChange={vi.fn()}
        onReact={vi.fn()}
      />,
    );
    for (const name of ['👍 でリアクション', 'スレッドで返信', 'リアクションを追加', '「後で」に保存']) {
      expect(screen.getByRole('button', { name })).toBeInTheDocument();
    }
    fireEvent.click(screen.getByRole('button', { name: 'その他の操作' }));
    expect(await screen.findByText('リマインド')).toBeInTheDocument();
    for (const name of [
      'ピン留め',
      '未読にする',
      'リンクをコピー',
      '20分後',
      '1時間後',
      '3時間後',
      '明日 9:00',
      '来週月曜 9:00',
      '転送',
      '編集',
      '削除',
    ]) {
      expect(screen.getByRole('button', { name })).toBeInTheDocument();
    }
  });
});
