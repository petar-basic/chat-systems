import { ConfirmDialog } from '@/shared/components/ConfirmDialog/ConfirmDialog';
import { toUserMessage } from '@/lib/errors';
import { useInstanceStore } from '@/stores/instances';
import type { Workspace } from '@/stores/workspace';
import { useLeaveWorkspace, useWorkspaceMembers } from './hooks/useWorkspaces';

interface Props {
  workspace: Workspace;
  onClose: () => void;
}

const LEAVING =
  'You leave every channel and stop getting its messages. To come back, somebody has to invite you again.';

export function LeaveWorkspaceDialog({ workspace, onClose }: Props) {
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
  };

  const owner = role === 'owner';
  const onlyOwner = owner && !members?.some((m) => m.role === 'owner' && m.user_id !== userId);

  return (
    <ConfirmDialog
      title={`Leave ${workspace.name}?`}
      description={
        onlyOwner
          ? `You're the only owner of ${workspace.name}. Make someone else an owner in Members first, then you can leave.`
          : owner
            ? `${LEAVING} The other owners keep running ${workspace.name}.`
            : LEAVING
      }
      confirmLabel="Leave workspace"
      confirmDisabled={!role || onlyOwner}
      onConfirm={confirm}
      onClose={onClose}
      dataQa="leave-workspace-dialog"
    />
  );
}
