import { useTranslation } from 'react-i18next';
import { ConfirmDialog } from '@/shared/components/ConfirmDialog/ConfirmDialog';
import { toUserMessage } from '@/lib/errors';
import { useInstanceStore } from '@/stores/instances';
import type { Workspace } from '@/stores/workspace';
import { useLeaveWorkspace, useWorkspaceMembers } from './hooks/useWorkspaces';

interface Props {
  workspace: Workspace;
  onClose: () => void;
}

export function LeaveWorkspaceDialog({ workspace, onClose }: Props) {
  const { t } = useTranslation('workspace');
  const userId = useInstanceStore((s) => s.instances.find((i) => i.url === workspace.instanceUrl)?.user.id);
  const { data: members } = useWorkspaceMembers(workspace.id, workspace.instanceUrl);
  const role = members?.find((m) => m.user_id === userId)?.role;
  const leave = useLeaveWorkspace();

  const confirm = async () => {
    if (!userId) return;
    try {
      await leave.mutateAsync({ workspace, userId });
    } catch (err) {
      throw new Error(toUserMessage(err, t('leave.failed')), { cause: err });
    }
  };

  const owner = role === 'owner';
  const onlyOwner = owner && !members?.some((m) => m.role === 'owner' && m.user_id !== userId);

  return (
    <ConfirmDialog
      title={t('leave.title', { name: workspace.name })}
      description={
        onlyOwner
          ? t('leave.onlyOwner', { name: workspace.name })
          : owner
            ? t('leave.bodyOwner', { name: workspace.name })
            : t('leave.body')
      }
      confirmLabel={t('leave.confirm')}
      confirmDisabled={!role || onlyOwner}
      onConfirm={confirm}
      onClose={onClose}
      dataQa="leave-workspace-dialog"
    />
  );
}
