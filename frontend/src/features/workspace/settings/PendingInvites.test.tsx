import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { PendingInvites } from './PendingInvites';
import { isPendingEmailInvite, type WorkspaceInvite } from '@/features/workspace/hooks/useWorkspaces';

const copy = vi.fn();
const revoke = vi.fn();
const later = new Date(Date.now() + 86_400_000).toISOString();
const invite = (extra: Partial<WorkspaceInvite>): WorkspaceInvite => ({
  id: 'i1',
  workspace_id: 'w1',
  created_by: 'u1',
  email: 'new@example.com',
  role: 'member',
  token: 't',
  max_uses: 1,
  use_count: 0,
  expires_at: later,
  created_at: later,
  ...extra,
});
let invites: WorkspaceInvite[] = [];

vi.mock('@/features/workspace/hooks/useWorkspaces', async (original) => ({
  ...(await original<typeof import('@/features/workspace/hooks/useWorkspaces')>()),
  useWorkspaceInvites: () => ({ data: invites }),
  useCopyInviteLink: () => ({ mutate: copy, isPending: false }),
  useRevokeInvite: () => ({ mutateAsync: revoke }),
}));

describe('PendingInvites', () => {
  beforeEach(() => {
    copy.mockReset();
    revoke.mockReset().mockResolvedValue(undefined);
  });

  it('lists only email invites still waiting to be used', () => {
    invites = [
      invite({}),
      invite({ id: 'used', email: 'used@example.com', use_count: 1 }),
      invite({ id: 'old', email: 'old@example.com', expires_at: '2020-01-01T00:00:00Z' }),
      invite({ id: 'link', email: null, max_uses: 10 }),
    ];
    render(<PendingInvites workspaceId="w1" />);

    expect(screen.getAllByTestId('pending-invite-row')).toHaveLength(1);
    expect(screen.getByText('new@example.com')).toBeInTheDocument();
  });

  it('copies the link and revokes after a confirmation', async () => {
    invites = [invite({})];
    render(<PendingInvites workspaceId="w1" />);

    fireEvent.click(screen.getByTestId('pending-invite-copy'));
    expect(copy).toHaveBeenCalledWith('i1', expect.anything());

    fireEvent.click(screen.getByTestId('pending-invite-revoke'));
    expect(revoke).not.toHaveBeenCalled();
    fireEvent.click(screen.getByTestId('pending-invite-revoke-dialog-confirm'));
    await waitFor(() => expect(revoke).toHaveBeenCalledWith('i1'));
  });

  it('shows nothing when no invite is pending', () => {
    invites = [];
    const { container } = render(<PendingInvites workspaceId="w1" />);
    expect(container).toBeEmptyDOMElement();
  });
});

describe('isPendingEmailInvite', () => {
  it('needs an address, time left and a use left', () => {
    expect(isPendingEmailInvite(invite({}))).toBe(true);
    expect(isPendingEmailInvite(invite({ email: null }))).toBe(false);
    expect(isPendingEmailInvite(invite({ use_count: 1 }))).toBe(false);
    expect(isPendingEmailInvite(invite({ expires_at: null }))).toBe(false);
  });
});
