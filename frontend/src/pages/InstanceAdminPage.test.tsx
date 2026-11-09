import { describe, it, expect, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/react';
import InstanceAdminPage from './InstanceAdminPage';

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
