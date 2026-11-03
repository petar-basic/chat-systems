import { useNavigate } from 'react-router';
import { ConfirmDialog } from '@/shared/components/ConfirmDialog/ConfirmDialog';
import { ROUTES } from '@/shared/constants';
import { toUserMessage } from '@/lib/errors';
import { useInstanceStore } from '@/stores/instances';
import { useWorkspaceStore, type Workspace } from '@/stores/workspace';
import { useLeaveWorkspace, useWorkspaceMembers } from './hooks/useWorkspaces';

interface Props {
  workspace: Workspace;
  onClose: () => void;
}

export function LeaveWorkspaceDialog({ workspace, onClose }: Props) {
  const navigate = useNavigate();
  const userId = useInstanceStore((s) => s.instances.find((i) => i.url === workspace.instanceUrl)?.user.id);
  const { data: members } = useWorkspaceMembers(workspace.id, workspace.instanceUrl);
  const role = members?.find((m) => m.user_id === userId)?.role;
  const leave = useLeaveWorkspace();

  const confirm = async () => {
    if (!userId) return;
    try {
      await leave.mutateAsync({ workspace, userId });
    } catch (err) {
      throw new Error(toUserMessage(err, 'Could not leave the workspace. Try again.'), { cause: err });
    }
    if (useWorkspaceStore.getState().currentWorkspace?.id !== workspace.id) return;
    navigate(ROUTES.app);
    useWorkspaceStore.setState({ currentWorkspace: null, currentChannel: null });
  };

  const owner = role === 'owner';

  return (
    <ConfirmDialog
      title={`Leave ${workspace.name}?`}
      description={
        owner
          ? `You own ${workspace.name}, and an owner can't leave it.`
          : 'You leave every channel and stop getting its messages. To come back, somebody has to invite you again.'
      }
      confirmLabel="Leave workspace"
      confirmDisabled={!role || owner}
      onConfirm={confirm}
      onClose={onClose}
      dataQa="leave-workspace-dialog"
    />
  );
}
