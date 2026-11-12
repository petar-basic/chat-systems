import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import i18n from '@/lib/i18n';
import { fireEvent, render, screen, within } from '@testing-library/react';
import InstanceAdminPage from './InstanceAdminPage';

const HOSTILE = '<img src=x onerror="alert(1)"> {{name}}';

const admin = vi.hoisted(() => ({
  mutate: vi.fn(),
  users: [
    {
      id: 'me',
      email: 'admin@example.com',
      display_name: 'Ada',
      status: 'active',
      instance_role: 'admin',
      created_at: '2026-11-01T10:00:00Z',
    },
    {
      id: 'u2',
      email: 'bo@example.com',
      display_name: 'Bo',
      status: 'active',
      instance_role: 'member',
      created_at: '2026-11-02T10:00:00Z',
    },
  ],
}));

vi.mock('react-router', () => ({ useNavigate: () => vi.fn() }));
vi.mock('@/features/instance/hooks/useAuth', () => ({
  useCurrentUser: () => ({ data: { id: 'me', instance_role: 'admin' } }),
}));
vi.mock('@/stores/instances', () => ({
  useInstanceStore: () => ({
    instances: [{ url: 'https://chat.example.com' }],
    activeInstanceUrl: 'https://chat.example.com',
  }),
}));
vi.mock('@/features/instance/hooks/useAdmin', () => ({
  useAdminUsers: () => ({ data: admin.users, isLoading: false, isFetching: false, error: null }),
  useAdminAuditLog: () => ({ data: [], isLoading: false, isFetching: false, error: null }),
  useAdminWorkspaces: () => ({ data: [], isLoading: false, isFetching: false, error: null }),
  useSetInstanceRole: () => ({ mutate: admin.mutate, isPending: false, error: null }),
  useSetUserStatus: () => ({ mutate: vi.fn(), isPending: false, error: null }),
}));
vi.mock('@/features/workspace/components/AuditLogTable', () => ({ default: () => null }));
vi.mock('@/features/setup/TestEmailCheck', () => ({ TestEmailCheck: () => null }));
vi.mock('@/features/setup/EmailCodeSetting', () => ({ EmailCodeSetting: () => null }));
vi.mock('@/features/setup/WorkspaceCreationSetting', () => ({
  WorkspaceCreationSetting: () => <p>who can create workspaces</p>,
}));

describe('InstanceAdminPage', () => {
  beforeEach(() => admin.mutate.mockReset());

  it('puts who can create workspaces next to the roles that decide it', () => {
    render(<InstanceAdminPage />);
    expect(screen.getByText('who can create workspaces')).toBeInTheDocument();
    const select = screen.getByLabelText('Instance role for Bo');
    expect(
      within(select)
        .getAllByRole('option')
        .map((o) => o.textContent),
    ).toEqual(['Member', 'Workspace creator', 'Instance admin']);
    expect(select).toHaveValue('member');
  });

  it('gives a member the workspace creator role', () => {
    render(<InstanceAdminPage />);
    fireEvent.change(screen.getByLabelText('Instance role for Bo'), {
      target: { value: 'workspace_creator' },
    });
    expect(admin.mutate).toHaveBeenCalledWith({ userId: 'u2', role: 'workspace_creator' });
  });

  it('does not let an admin change their own role', () => {
    render(<InstanceAdminPage />);
    expect(screen.getByLabelText('Instance role for Ada')).toBeDisabled();
  });
});

describe('InstanceAdminPage in Japanese', () => {
  const people = admin.users;

  beforeEach(() => {
    admin.users = [
      ...people,
      {
        id: 'u3',
        email: 'x@example.com',
        display_name: HOSTILE,
        status: 'suspended',
        instance_role: 'member',
        created_at: '2026-11-03T10:00:00Z',
      },
      {
        id: 'u4',
        email: 'new@example.com',
        display_name: '',
        status: 'pending',
        instance_role: 'member',
        created_at: '2026-11-04T10:00:00Z',
      },
    ];
  });

  afterEach(async () => {
    admin.users = people;
    await i18n.changeLanguage('en');
  });

  it('shows the users, their state and roles, and names stay plain text', async () => {
    await i18n.changeLanguage('ja');
    render(<InstanceAdminPage />);

    expect(screen.getByRole('heading', { name: 'サーバー管理' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'アプリに戻る' })).toBeInTheDocument();
    for (const tab of ['users', 'workspaces', 'audit', 'email']) {
      expect(screen.getByTestId(`admin-tab-${tab}`)).toBeInTheDocument();
    }
    expect(screen.getByTestId('admin-tab-audit')).toHaveTextContent('監査ログ');
    expect(screen.getByText('サーバーのユーザー: 4人')).toBeInTheDocument();
    expect(screen.getAllByRole('columnheader').map((h) => h.textContent)).toEqual([
      'ユーザー',
      '状態',
      '役割',
      '操作',
    ]);
    expect(screen.getAllByText('有効')).toHaveLength(2);
    expect(screen.getByText('停止中')).toBeInTheDocument();
    expect(screen.getByText('招待中')).toBeInTheDocument();
    expect(screen.getByText('（名前なし）')).toBeInTheDocument();
    expect(screen.getByText('（自分）')).toBeInTheDocument();

    const select = screen.getByLabelText('Bo さんのサーバーでの役割');
    expect(
      within(select)
        .getAllByRole('option')
        .map((o) => o.textContent),
    ).toEqual(['メンバー', 'ワークスペース作成者', 'サーバー管理者']);

    expect(screen.getByLabelText(`${HOSTILE} さんのサーバーでの役割`)).toBeInTheDocument();
    expect(screen.getByText(HOSTILE, { exact: true })).toBeInTheDocument();
    expect(screen.getAllByRole('button', { name: '停止する' }).length).toBeGreaterThan(0);
    const activate = screen.getAllByRole('button', { name: '有効にする' });
    expect(activate).toHaveLength(2);
    expect(activate[0]).toHaveAttribute('data-tooltip', 'ユーザーを有効にする');
    expect(document.querySelector('[onerror]')).toBeNull();
  });
});
