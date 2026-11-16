import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import i18n from '@/lib/i18n';
import type { WorkspaceMember } from '@/stores/workspace';
import { GuestSeatDialog } from './GuestSeatDialog';

const hostile = '<img src=x onerror="alert(1)"> {{name}}';

const update = vi.hoisted(() => ({ mutateAsync: vi.fn(), isPending: false }));

vi.mock('@/features/workspace/hooks/useWorkspaces', () => ({
  useUpdateGuest: () => update,
}));

const guest = {
  user_id: 'u1',
  role: 'guest',
  guest_kind: 'single',
  guest_until: '2026-12-01T23:59:59Z',
} as WorkspaceMember;

describe('GuestSeatDialog in Japanese', () => {
  beforeEach(async () => {
    await i18n.changeLanguage('ja');
  });

  afterEach(async () => {
    update.mutateAsync.mockReset();
    await i18n.changeLanguage('en');
  });

  it('names the guest as text and explains the end date', () => {
    render(<GuestSeatDialog workspaceId="w1" member={guest} name={hostile} onClose={vi.fn()} />);

    expect(screen.getByText(`${hostile} さんのゲストとしてのアクセス`)).toBeInTheDocument();
    expect(screen.getByRole('option', { name: '1チャンネルのみのゲスト' })).toBeInTheDocument();
    expect(screen.getByRole('option', { name: '複数チャンネルのゲスト' })).toBeInTheDocument();
    expect(
      screen.getByText('その日にワークスペースから外れます。管理者には3日前に知らせが届きます。'),
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '終了日なし' })).toBeInTheDocument();
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  it('says in Japanese when the change fails without a reason', async () => {
    update.mutateAsync.mockRejectedValue(new Error(''));
    render(<GuestSeatDialog workspaceId="w1" member={guest} name="Ana" onClose={vi.fn()} />);

    fireEvent.click(screen.getByRole('button', { name: '保存' }));

    expect(await screen.findByText('ゲストのアクセスを変更できませんでした')).toBeInTheDocument();
  });
});
