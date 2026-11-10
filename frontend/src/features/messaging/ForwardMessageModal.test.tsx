import { describe, it, expect, vi, afterEach } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import i18n from '@/lib/i18n';
import type { Channel } from '@/stores/workspace';
import ForwardMessageModal from './ForwardMessageModal';

vi.mock('@/shared/hooks/useCurrentApi', () => ({ getApiForInstance: vi.fn() }));

const source = {
  messageId: 'm-1',
  content: 'shipping now',
  authorName: 'Ana',
  origin: '#general',
  hasAttachments: true,
};

describe('ForwardMessageModal in Japanese', () => {
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('names the dialog and its fields, and says when nothing matches', async () => {
    await i18n.changeLanguage('ja');
    render(
      <ForwardMessageModal
        source={source}
        channels={[{ id: 'ch-1', name: 'design' } as unknown as Channel]}
        conversations={[]}
        currentUserId="me"
        onClose={vi.fn()}
      />,
    );
    expect(screen.getByRole('heading', { name: 'メッセージを転送' })).toBeInTheDocument();
    expect(screen.getByText('📎 ファイルも一緒に転送されます')).toBeInTheDocument();
    expect(screen.getByLabelText(/コメント/)).toHaveAttribute('placeholder', 'コメントを追加（任意）');
    const search = screen.getByLabelText('転送先を検索');
    expect(search).toHaveAttribute('placeholder', 'チャンネルやメンバーを検索');
    fireEvent.change(search, { target: { value: 'nowhere' } });
    expect(screen.getByText('該当するものがありません。')).toBeInTheDocument();
  });

  it('in Japanese says who wrote it and where, with both as text', async () => {
    const hostile = '<img src=x onerror="alert(1)"> {{name}}';
    await i18n.changeLanguage('ja');
    render(
      <ForwardMessageModal
        source={{ ...source, authorName: hostile, origin: `#${hostile}` }}
        channels={[]}
        conversations={[]}
        currentUserId="me"
        onClose={vi.fn()}
      />,
    );
    expect(screen.getByTestId('forward-preview').previousElementSibling?.textContent).toBe(
      `投稿者: ${hostile} さん ・ 投稿先: #${hostile}`,
    );
    expect(document.querySelector('[onerror]')).toBeNull();
  });
});
