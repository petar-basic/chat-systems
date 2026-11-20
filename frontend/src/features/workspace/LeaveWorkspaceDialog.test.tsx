import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import i18n from '@/lib/i18n';
import { useInstanceStore } from '@/stores/instances';
import type { Workspace } from '@/stores/workspace';
import { LeaveWorkspaceDialog } from './LeaveWorkspaceDialog';

const hostile = '<img src=x onerror="alert(1)"> {{name}}';

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

  it('says what leaving does', () => {
    render(<LeaveWorkspaceDialog workspace={northwind} onClose={vi.fn()} />);
    expect(screen.getByRole('heading', { name: 'Leave Northwind?' })).toBeInTheDocument();
    expect(
      screen.getByText(
        'You leave every channel and stop getting its messages. To come back, somebody has to invite you again.',
      ),
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Leave workspace' })).toBeInTheDocument();
  });

  it('says so when leaving fails with no reason', async () => {
    leave.mockRejectedValue(new Error(''));
    render(<LeaveWorkspaceDialog workspace={northwind} onClose={vi.fn()} />);
    fireEvent.click(screen.getByTestId('leave-workspace-dialog-confirm'));
    expect(await screen.findByRole('alert')).toHaveTextContent('Could not leave the workspace. Try again.');
  });

  it.each([
    ['a member', 'member', [], `Leave ${hostile}?`, null],
    [
      'one of two owners',
      'owner',
      [{ user_id: 'ana', role: 'owner' }],
      `Leave ${hostile}?`,
      `You leave every channel and stop getting its messages. To come back, somebody has to invite you again. The other owners keep running ${hostile}.`,
    ],
    [
      'the only owner',
      'owner',
      [],
      `Leave ${hostile}?`,
      `You're the only owner of ${hostile}. Make someone else an owner in Members first, then you can leave.`,
    ],
  ])('shows the workspace name as text to %s', (_who, myRole, owners, title, body) => {
    role = myRole;
    others = owners;
    render(<LeaveWorkspaceDialog workspace={{ ...northwind, name: hostile }} onClose={vi.fn()} />);
    expect(screen.getByRole('heading').textContent).toBe(title);
    if (body) expect(screen.getByText(body)).toBeInTheDocument();
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  describe('in Japanese', () => {
    beforeEach(async () => {
      await i18n.changeLanguage('ja');
    });

    afterEach(async () => {
      await i18n.changeLanguage('en');
    });

    it('says what leaving does', () => {
      render(<LeaveWorkspaceDialog workspace={northwind} onClose={vi.fn()} />);
      expect(screen.getByRole('heading', { name: 'Northwind から退出しますか？' })).toBeInTheDocument();
      expect(
        screen.getByText(
          'すべてのチャンネルから退出し、メッセージも届かなくなります。戻るには、もう一度招待してもらう必要があります。',
        ),
      ).toBeInTheDocument();
      expect(screen.getByRole('button', { name: 'ワークスペースから退出' })).toBeInTheDocument();
    });

    it('tells one of two owners the others keep running it', () => {
      role = 'owner';
      others = [{ user_id: 'ana', role: 'owner' }];
      render(<LeaveWorkspaceDialog workspace={northwind} onClose={vi.fn()} />);
      expect(
        screen.getByText(
          'すべてのチャンネルから退出し、メッセージも届かなくなります。戻るには、もう一度招待してもらう必要があります。Northwind は、ほかのオーナーが引き続き管理します。',
        ),
      ).toBeInTheDocument();
    });

    it('tells the only owner to make another owner first', () => {
      role = 'owner';
      others = [{ user_id: 'ana', role: 'admin' }];
      render(<LeaveWorkspaceDialog workspace={northwind} onClose={vi.fn()} />);
      expect(
        screen.getByText(
          'あなたは Northwind の唯一のオーナーです。先にワークスペースの設定 → メンバーで別の人をオーナーにすると、退出できるようになります。',
        ),
      ).toBeInTheDocument();
      expect(screen.getByTestId('leave-workspace-dialog-confirm')).toBeDisabled();
    });

    it('says so when leaving fails with no reason', async () => {
      leave.mockRejectedValue(new Error(''));
      render(<LeaveWorkspaceDialog workspace={northwind} onClose={vi.fn()} />);
      fireEvent.click(screen.getByTestId('leave-workspace-dialog-confirm'));
      expect(await screen.findByRole('alert')).toHaveTextContent(
        'ワークスペースから退出できませんでした。もう一度お試しください。',
      );
    });

    it('has Japanese for every one', () => {
      const english = Object.keys(i18n.getResource('en', 'workspace', 'leave'));
      const japanese = Object.keys(i18n.getResource('ja', 'workspace', 'leave'));
      expect(english.length).toBeGreaterThan(0);
      expect(japanese).toEqual(english);
    });
  });
});
