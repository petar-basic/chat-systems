import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import i18n from '@/lib/i18n';
import type { WorkspaceMember } from '@/stores/workspace';
import { NewDmModal } from './NewDmModal';

const hostile = '<img src=x onerror="alert(1)"> {{name}}';

const member = (user_id: string, display_name: string) =>
  ({
    user_id,
    display_name,
    email: `${user_id}@x.test`,
    avatar_url: null,
    title: null,
    pronouns: null,
  }) as unknown as WorkspaceMember;

const members = [member('u-1', 'Me'), member('u-2', hostile), member('u-3', 'Bea')];

describe('NewDmModal', () => {
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('in Japanese counts who is picked and names the start button, with names as text', async () => {
    const onStart = vi.fn();
    await i18n.changeLanguage('ja');
    render(<NewDmModal members={members} currentUserId="u-1" onStart={onStart} onClose={vi.fn()} />);

    expect(screen.getByRole('heading', { name: '新規メッセージ' })).toBeInTheDocument();
    expect(screen.getByTestId('new-dm-selected-count')).toHaveTextContent('まだ選ばれていません');
    expect(screen.getByText(hostile)).toBeInTheDocument();
    expect(document.querySelector('img')).toBeNull();

    const [first, second] = screen.getAllByTestId('new-dm-candidate');
    fireEvent.click(first);
    expect(screen.getByTestId('new-dm-start')).toHaveTextContent('開始');
    fireEvent.click(second);
    expect(screen.getByTestId('new-dm-selected-count')).toHaveTextContent('2人を選択中');
    expect(screen.getByTestId('new-dm-start')).toHaveTextContent('グループチャット開始');
    expect(screen.getByRole('button', { name: 'キャンセル' })).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('メンバー検索'), { target: { value: 'nobody-here' } });
    expect(screen.getByText('該当するメンバーがいません')).toBeInTheDocument();
  });
});
