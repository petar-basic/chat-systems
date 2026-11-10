import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor, within } from '@testing-library/react';
import type { AdminWorkspace, WorkspacePerson } from './hooks/useAdmin';
import { AdminWorkspaces } from './AdminWorkspaces';

const appoint = vi.fn();
const person = (user_id: string, role: WorkspacePerson['role'], status: WorkspacePerson['status']) => ({
  user_id,
  display_name: user_id,
  email: `${user_id}@x.test`,
  role,
  status,
});

const orphaned: AdminWorkspace = {
  id: 'w1',
  name: 'Northwind',
  slug: 'northwind',
  owner_id: 'olga',
  is_active: true,
  created_at: '2026-11-01T10:00:00Z',
  owners: [person('olga', 'owner', 'suspended')],
};
const healthy: AdminWorkspace = {
  ...orphaned,
  id: 'w2',
  name: 'Acme',
  owners: [person('ana', 'owner', 'active')],
};

vi.mock('./hooks/useAdmin', () => ({
  useAdminWorkspaces: () => ({ data: [orphaned, healthy], isLoading: false, error: null }),
  useAdminWorkspacePeople: () => ({
    data: [
      person('olga', 'owner', 'suspended'),
      person('maya', 'member', 'active'),
      person('sam', 'admin', 'suspended'),
    ],
    isLoading: false,
  }),
  useAppointOwner: () => ({ mutateAsync: appoint }),
}));

describe('AdminWorkspaces', () => {
  beforeEach(() => {
    appoint.mockReset().mockResolvedValue(undefined);
  });

  it('flags a workspace no owner can sign in to', () => {
    render(<AdminWorkspaces instanceUrl="https://chat.example" />);

    const [first, second] = screen.getAllByTestId('admin-workspace-row');
    expect(within(first).getByText('suspended')).toBeInTheDocument();
    expect(within(first).getByTestId('admin-workspace-stranded')).toHaveTextContent('No owner can sign in');
    expect(within(second).queryByTestId('admin-workspace-stranded')).not.toBeInTheDocument();
  });

  it('makes an active member the owner', async () => {
    render(<AdminWorkspaces instanceUrl="https://chat.example" />);

    fireEvent.click(
      within(screen.getAllByTestId('admin-workspace-row')[0]).getByTestId('admin-workspace-appoint'),
    );
    const options = within(screen.getByTestId('appoint-owner-select')).getAllByRole('option');
    expect(options.map((o) => o.getAttribute('value'))).toEqual(['maya']);
    fireEvent.click(screen.getByTestId('appoint-owner-dialog-confirm'));

    await waitFor(() => expect(appoint).toHaveBeenCalledWith({ workspaceId: 'w1', userId: 'maya' }));
  });
});
