import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { useInstanceStore } from '@/stores/instances';
import type { Workspace } from '@/stores/workspace';
import { LeaveWorkspaceDialog } from './LeaveWorkspaceDialog';

const leave = vi.fn();
let role = 'member';
let others: { user_id: string; role: string }[] = [];

vi.mock('./hooks/useWorkspaces', async (original) => ({
  ...(await original<typeof import('./hooks/useWorkspaces')>()),
  useWorkspaceMembers: () => ({ data: [{ user_id: 'me', role }, ...others] }),
  useLeaveWorkspace: () => ({ mutateAsync: leave }),
}));

const northwind = { id: 'w1', name: 'Northwind', instanceUrl: 'https://chat.example.com' } as Workspace;

describe('LeaveWorkspaceDialog', () => {
  beforeEach(() => {
    leave.mockReset().mockResolvedValue(undefined);
    role = 'member';
    others = [];
    useInstanceStore.setState({
      instances: [{ url: 'https://chat.example.com', user: { id: 'me' } }],
    } as never);
  });

  it('leaves the workspace', async () => {
    const onClose = vi.fn();
    render(<LeaveWorkspaceDialog workspace={northwind} onClose={onClose} />);

    fireEvent.click(screen.getByTestId('leave-workspace-dialog-confirm'));

    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(leave).toHaveBeenCalledWith({ workspace: northwind, userId: 'me' });
  });

  it('tells the only owner to make another owner first', () => {
    role = 'owner';
    others = [{ user_id: 'ana', role: 'admin' }];
    render(<LeaveWorkspaceDialog workspace={northwind} onClose={vi.fn()} />);

    expect(
      screen.getByText(
        "You're the only owner of Northwind. Make someone else an owner in Members first, then you can leave.",
      ),
    ).toBeInTheDocument();
    expect(screen.getByTestId('leave-workspace-dialog-confirm')).toBeDisabled();
  });

  it('lets one of two owners leave', async () => {
    role = 'owner';
    others = [{ user_id: 'ana', role: 'owner' }];
    const onClose = vi.fn();
    render(<LeaveWorkspaceDialog workspace={northwind} onClose={onClose} />);

    expect(screen.getByText(/The other owners keep running Northwind\./)).toBeInTheDocument();
    fireEvent.click(screen.getByTestId('leave-workspace-dialog-confirm'));

    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(leave).toHaveBeenCalledWith({ workspace: northwind, userId: 'me' });
  });

  it('stays open with the reason when the server refuses', async () => {
    leave.mockRejectedValue(new Error('Make someone else an owner first'));
    const onClose = vi.fn();
    render(<LeaveWorkspaceDialog workspace={northwind} onClose={onClose} />);

    fireEvent.click(screen.getByTestId('leave-workspace-dialog-confirm'));

    expect(await screen.findByRole('alert')).toHaveTextContent('Make someone else an owner first');
    expect(onClose).not.toHaveBeenCalled();
  });
});
