import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import i18n from '@/lib/i18n';
import { render, screen } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import ChannelMembersPanel from './ChannelMembersPanel';

const hostile = '<img src=x onerror="alert(1)"> {{name}}';
const names: Record<string, string> = { 'u-me': 'Me', 'u-x': hostile, 'u-out': 'Outsider' };

vi.mock('@/stores/users', () => ({
  useUserCache: () => ({
    users: new Map(Object.keys(names).map((id) => [id, {}])),
    getUser: (id: string) => ({ display_name: names[id] }),
  }),
}));
vi.mock('@/features/channel/hooks/useChannelModeration', () => ({
  useChannelModeration: () => ({
    members: [
      { user_id: 'u-me', role: 'admin' },
      { user_id: 'u-x', role: 'member' },
    ],
    isLoading: false,
    canModerate: true,
    myRole: 'admin',
    workspaceRole: 'member',
    currentUserId: 'u-me',
  }),
}));
vi.mock('@/lib/channelArchive', () => ({ useChannelArchived: () => false }));
vi.mock('./hooks/useChannels', () => ({
  useUpdateChannelMemberRole: () => ({ mutate: vi.fn(), isPending: false }),
}));

describe('ChannelMembersPanel in Japanese', () => {
  beforeEach(async () => {
    await i18n.changeLanguage('ja');
  });
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('names the channel, members and what can be done to them, as text', () => {
    render(
      <QueryClientProvider client={new QueryClient()}>
        <ChannelMembersPanel
          channelId="c1"
          channelName={hostile}
          channelType="public"
          startAdding
          onClose={vi.fn()}
        />
      </QueryClientProvider>,
    );
    expect(screen.getByText(`${hostile} のメンバー`)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'メンバー一覧を閉じる' })).toBeInTheDocument();
    expect(screen.getByText('メンバー（2）')).toBeInTheDocument();
    expect(screen.getByText('（自分）')).toBeInTheDocument();
    expect(screen.getAllByTestId('channel-member-role').map((role) => role.textContent)).toEqual([
      'チャンネル管理者',
      'メンバー',
    ]);
    expect(
      screen.getByRole('button', { name: `${hostile} さんをチャンネル管理者にする` }),
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: `${hostile} さんをチャンネルから外す` })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'チャンネルから退出' })).toBeInTheDocument();
    expect(screen.getByText('チャンネルに追加')).toBeInTheDocument();
    expect(document.querySelector('[onerror]')).toBeNull();
  });
});
