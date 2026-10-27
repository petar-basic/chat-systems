import type { Channel, WorkspaceMember, Workspace } from '@/stores/workspace';
import type { Conversation } from '@/features/messaging';
import type { RightPanel } from './hooks/useRightPanel';
import { ThreadPanel } from '@/features/threads';
import { SearchPanel } from '@/features/search';
import { PinnedMessagesPanel, ChannelFilesPanel, ChannelMembersPanel } from '@/features/channel';
import { NotificationsPanel } from '@/features/notifications';
import { useCurrentWorkspaceRole } from './hooks/useCurrentWorkspaceRole';
import { ScheduledMessagesPanel } from '@/features/messaging';
import { LaterPanel } from '@/features/later';
import { WorkflowsPanel, canBuild, workflowBuildersOf } from '@/features/workflows';

interface Props {
  panel: RightPanel;
  currentWorkspace: Workspace | null;
  currentChannel: Channel | null;
  workspaceMembers: WorkspaceMember[];
  channels: Channel[];
  conversations: Conversation[];
  currentUserId?: string;
  onClose: () => void;
  onOpenMessage: (messageId: string, thread?: boolean) => void;
}

export default function WorkspaceRightPanels({
  panel,
  currentWorkspace,
  currentChannel,
  workspaceMembers,
  channels,
  conversations,
  currentUserId,
  onClose,
  onOpenMessage,
}: Props) {
  const { role } = useCurrentWorkspaceRole();

  if (!panel) return null;

  if (panel.kind === 'thread') {
    return (
      <ThreadPanel
        parentMessage={panel.message}
        replyId={panel.replyId}
        members={workspaceMembers}
        channels={channels}
        onClose={onClose}
      />
    );
  }
  if (panel.kind === 'later' && currentWorkspace) {
    return (
      <LaterPanel
        workspaceId={currentWorkspace.id}
        instanceUrl={currentWorkspace.instanceUrl}
        channels={channels}
        conversations={conversations}
        onClose={onClose}
        onOpenMessage={onOpenMessage}
      />
    );
  }
  if (panel.kind === 'search' && currentWorkspace) {
    return <SearchPanel members={workspaceMembers} onClose={onClose} onOpenMessage={onOpenMessage} />;
  }
  if (panel.kind === 'pins' && currentChannel) {
    return (
      <PinnedMessagesPanel
        channelId={currentChannel.id}
        onClose={onClose}
        onNavigate={(msgId) => onOpenMessage(msgId)}
      />
    );
  }
  if (panel.kind === 'files' && currentChannel) {
    return (
      <ChannelFilesPanel
        channelId={currentChannel.id}
        onClose={onClose}
        onNavigate={(msgId) => onOpenMessage(msgId)}
      />
    );
  }
  if (panel.kind === 'channelMembers' && currentChannel && currentWorkspace) {
    return (
      <ChannelMembersPanel
        key={panel.adding ? 'adding' : 'list'}
        startAdding={panel.adding}
        channelId={currentChannel.id}
        channelName={currentChannel.name ?? ''}
        channelType={currentChannel.channel_type}
        onClose={onClose}
      />
    );
  }
  if (panel.kind === 'notifications' && currentWorkspace) {
    return (
      <NotificationsPanel workspaceId={currentWorkspace.id} onClose={onClose} onOpenMessage={onOpenMessage} />
    );
  }
  if (panel.kind === 'workflows' && currentWorkspace && currentUserId) {
    return (
      <WorkflowsPanel
        key={panel.workflowId ?? 'list'}
        workflowId={panel.workflowId}
        workspaceId={currentWorkspace.id}
        instanceUrl={currentWorkspace.instanceUrl}
        channels={channels}
        members={workspaceMembers}
        currentUserId={currentUserId}
        canBuild={canBuild(role, workflowBuildersOf(currentWorkspace.settings))}
        onClose={onClose}
      />
    );
  }
  if (panel.kind === 'scheduled' && currentWorkspace) {
    return (
      <ScheduledMessagesPanel
        workspaceId={currentWorkspace.id}
        instanceUrl={currentWorkspace.instanceUrl}
        channels={channels}
        conversations={conversations}
        onClose={onClose}
      />
    );
  }
  return null;
}
