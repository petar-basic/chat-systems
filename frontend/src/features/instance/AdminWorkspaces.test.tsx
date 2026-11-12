import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor, within } from '@testing-library/react';
import type { AdminWorkspace, WorkspacePerson } from './hooks/useAdmin';
import { AdminWorkspaces } from './AdminWorkspaces';
import i18n from '@/lib/i18n';

const HOSTILE = '<img src=x onerror="alert(1)"> {{name}}';

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

describe('AdminWorkspaces in Japanese', () => {
  const name = orphaned.name;

  beforeEach(() => {
    orphaned.name = HOSTILE;
  });

  afterEach(async () => {
    orphaned.name = name;
    await i18n.changeLanguage('en');
  });

  it('flags the stranded workspace and asks who to make an owner', async () => {
    await i18n.changeLanguage('ja');
    render(<AdminWorkspaces instanceUrl="https://chat.example" />);

    expect(
      screen.getByText(
        'サーバーのワークスペース: 2件。サインインできるオーナーがいなくなったワークスペースでは、メンバーの1人をオーナーにしてください。',
      ),
    ).toBeInTheDocument();
    expect(screen.getAllByRole('columnheader').map((h) => h.textContent)).toEqual([
      'ワークスペース',
      'オーナー',
      '操作',
    ]);
    const [first] = screen.getAllByTestId('admin-workspace-row');
    expect(within(first).getByText(HOSTILE, { exact: true })).toBeInTheDocument();
    expect(within(first).getByText('停止中')).toBeInTheDocument();
    expect(within(first).getByTestId('admin-workspace-stranded')).toHaveTextContent(
      'サインインできるオーナーがいません',
    );

    fireEvent.click(within(first).getByTestId('admin-workspace-appoint'));
    expect(within(first).getByTestId('admin-workspace-appoint')).toHaveTextContent('オーナーを追加');
    const dialog = screen.getByTestId('appoint-owner-dialog');
    expect(within(dialog).getByRole('heading').textContent).toBe(`${HOSTILE} にオーナーを追加`);
    expect(
      within(dialog).getByText(
        'オーナーは、データの保持期間とエクスポートを管理し、ワークスペースを削除することもできます。今のオーナーは、そのままオーナーとして残ります。',
      ),
    ).toBeInTheDocument();
    expect(within(dialog).getByRole('option').textContent).toBe('maya (maya@x.test) · メンバー');
    expect(screen.getByTestId('appoint-owner-dialog-confirm')).toHaveTextContent('オーナーにする');
    expect(document.querySelector('[onerror]')).toBeNull();
  });
});
