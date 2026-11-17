import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ReactNode } from 'react';
import { fireEvent, render, screen } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import i18n from '@/lib/i18n';
import { useWorkspaceStore, type Workspace, type WorkspaceMember } from '@/stores/workspace';
import { MembersSection } from './MembersSection';

const hostile = '<img src=x onerror="alert(1)"> {{name}}';

const state = vi.hoisted(() => ({
  role: 'owner' as string | null,
  members: [] as WorkspaceMember[],
  patch: vi.fn(),
}));

vi.mock('@/features/workspace/hooks/useCurrentWorkspaceRole', () => ({
  useCurrentWorkspaceRole: () => ({ role: state.role }),
}));
vi.mock('@/features/workspace/hooks/useWorkspaces', async (original) => ({
  ...(await original<typeof import('@/features/workspace/hooks/useWorkspaces')>()),
  useWorkspaceMembers: () => ({ data: state.members, isLoading: false }),
  useWorkspaceInvites: () => ({ data: [] }),
}));
vi.mock('@/features/workspace/hooks/useInviteRequests', async (original) => ({
  ...(await original<typeof import('@/features/workspace/hooks/useInviteRequests')>()),
  useInviteRequests: () => ({ data: [] }),
}));
vi.mock('@/features/workspace/hooks/useExports', () => ({
  useStartPersonExport: () => ({ mutate: vi.fn(), isPending: false }),
  useErasePerson: () => ({ mutateAsync: vi.fn() }),
}));
vi.mock('@/shared/hooks/useCurrentApi', () => ({
  getApiForInstance: () => ({
    typed: (call: (c: { PATCH: typeof state.patch; DELETE: typeof state.patch }) => unknown) =>
      call({ PATCH: state.patch, DELETE: state.patch }),
  }),
}));

const member = (over: Partial<WorkspaceMember>): WorkspaceMember =>
  ({
    user_id: 'u-ana',
    display_name: 'Ana',
    email: 'ana@example.com',
    role: 'member',
    is_bot: false,
    ...over,
  }) as WorkspaceMember;

function wrapper({ children }: { children: ReactNode }) {
  return <QueryClientProvider client={new QueryClient()}>{children}</QueryClientProvider>;
}

function renderSection() {
  render(<MembersSection workspaceId="w1" openInvite={false} isInstanceAdmin={false} />, { wrapper });
}

describe('MembersSection in Japanese', () => {
  beforeEach(async () => {
    state.role = 'owner';
    state.patch.mockReset().mockResolvedValue({ data: {} });
    useWorkspaceStore.setState({
      currentUserId: 'u-me',
      currentWorkspace: { id: 'w1', name: 'Dev', settings: {} } as unknown as Workspace,
    });
    await i18n.changeLanguage('ja');
  });

  afterEach(async () => {
    useWorkspaceStore.setState({ currentUserId: null, currentWorkspace: null });
    await i18n.changeLanguage('en');
  });

  it('lists the members with their roles, and names them as text', () => {
    state.members = [
      member({ user_id: 'u-me', display_name: 'Me', role: 'owner' }),
      member({ display_name: hostile }),
      member({ user_id: 'u-bot', display_name: 'Deploy', is_bot: true }),
    ];
    renderSection();

    expect(screen.getByRole('button', { name: 'メンバーを招待' })).toBeInTheDocument();
    expect(screen.getByText('メンバー: 3人')).toBeInTheDocument();
    expect(
      screen.getByText(
        'オーナーはあなただけです。降りたり抜けたりするには、先にほかの人をオーナーにしてください。その人の行で「オーナー」を選びます。',
      ),
    ).toBeInTheDocument();
    expect(screen.getByText('アプリ')).toBeInTheDocument();
    const select = screen.getByLabelText(`${hostile} さんの役割`);
    expect([...select.querySelectorAll('option')].map((o) => o.textContent)).toEqual([
      'ゲスト',
      'メンバー',
      '管理者',
      'オーナー',
    ]);
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  it('asks before removing someone', () => {
    state.members = [member({ display_name: hostile })];
    renderSection();

    fireEvent.click(screen.getByRole('button', { name: `${hostile} さんを外す` }));
    expect(screen.getByText('ワークスペースから外しますか?')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '外す' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'キャンセル' })).toBeInTheDocument();
  });

  it('lets an owner step down when another owner remains', () => {
    state.members = [
      member({ user_id: 'u-me', display_name: 'Me', role: 'owner' }),
      member({ user_id: 'u-bo', display_name: 'Bo', role: 'owner' }),
    ];
    renderSection();

    expect(
      screen.getByText(
        'ほかにもオーナーがいるので、管理者に降りることも、ワークスペースから抜けることもできます。',
      ),
    ).toBeInTheDocument();
    expect(screen.getAllByText('オーナー')).toHaveLength(2);
    fireEvent.click(screen.getByRole('button', { name: 'オーナーを降りる' }));
    expect(
      screen.getByText(
        'オーナーを降りて管理者になりますか? もう一度オーナーになるには、ほかのオーナーに戻してもらう必要があります。',
      ),
    ).toBeInTheDocument();
  });

  it('says in Japanese when a role change fails without a reason', async () => {
    state.patch.mockRejectedValue(new Error(''));
    state.members = [member({})];
    renderSection();

    fireEvent.change(screen.getByLabelText('Ana さんの役割'), { target: { value: 'admin' } });
    expect(await screen.findByText('役割を変更できませんでした')).toBeInTheDocument();
  });
});

describe('workspace settings strings', () => {
  function leaves(value: unknown, prefix = ''): string[] {
    if (typeof value !== 'object' || value === null) return [prefix];
    return Object.entries(value).flatMap(([key, child]) => leaves(child, prefix ? `${prefix}.${key}` : key));
  }
  const counted = (keys: string[]) =>
    [...new Set(keys.map((key) => key.replace(/_(one|other)$/, '')))].sort();

  it('has Japanese for every one', () => {
    const english = leaves(i18n.getResource('en', 'workspace', 'settings'));
    const japanese = leaves(i18n.getResource('ja', 'workspace', 'settings'));
    expect(counted(japanese)).toEqual(counted(english));
  });
});
