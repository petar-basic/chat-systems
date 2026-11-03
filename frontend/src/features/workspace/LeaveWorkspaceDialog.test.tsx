import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { useInstanceStore } from '@/stores/instances';
import { useWorkspaceStore, type Workspace } from '@/stores/workspace';
import { LeaveWorkspaceDialog } from './LeaveWorkspaceDialog';

const leave = vi.fn();
const navigate = vi.fn();
let role = 'member';

vi.mock('react-router', async (original) => ({
  ...(await original<typeof import('react-router')>()),
  useNavigate: () => navigate,
}));

vi.mock('./hooks/useWorkspaces', async (original) => ({
  ...(await original<typeof import('./hooks/useWorkspaces')>()),
  useWorkspaceMembers: () => ({ data: [{ user_id: 'me', role }] }),
  useLeaveWorkspace: () => ({ mutateAsync: leave }),
}));

const northwind = { id: 'w1', name: 'Northwind', instanceUrl: 'https://chat.example.com' } as Workspace;

describe('LeaveWorkspaceDialog', () => {
  beforeEach(() => {
    leave.mockReset().mockResolvedValue(undefined);
    navigate.mockReset();
    role = 'member';
    useInstanceStore.setState({
      instances: [{ url: 'https://chat.example.com', user: { id: 'me' } }],
    } as never);
    useWorkspaceStore.setState({ currentWorkspace: northwind });
  });

  it('leaves the workspace and moves off it', async () => {
    const onClose = vi.fn();
    render(<LeaveWorkspaceDialog workspace={northwind} onClose={onClose} />);

    fireEvent.click(screen.getByTestId('leave-workspace-dialog-confirm'));

    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(leave).toHaveBeenCalledWith({ workspace: northwind, userId: 'me' });
    expect(navigate).toHaveBeenCalledWith('/app');
    expect(useWorkspaceStore.getState().currentWorkspace).toBeNull();
  });

  it('tells an owner they cannot leave', () => {
    role = 'owner';
    render(<LeaveWorkspaceDialog workspace={northwind} onClose={vi.fn()} />);

    expect(screen.getByText("You own Northwind, and an owner can't leave it.")).toBeInTheDocument();
    expect(screen.getByTestId('leave-workspace-dialog-confirm')).toBeDisabled();
  });

  it('stays open with the reason when the server refuses', async () => {
    leave.mockRejectedValue(new Error('The workspace owner cannot be removed'));
    const onClose = vi.fn();
    render(<LeaveWorkspaceDialog workspace={northwind} onClose={onClose} />);

    fireEvent.click(screen.getByTestId('leave-workspace-dialog-confirm'));

    expect(await screen.findByRole('alert')).toHaveTextContent('The workspace owner cannot be removed');
    expect(onClose).not.toHaveBeenCalled();
    expect(navigate).not.toHaveBeenCalled();
  });
});
