import type { Channel, WorkspaceMember, Workspace } from '@/stores/workspace';
import type { Conversation } from '@/features/messaging/hooks/useConversations';
import type { RightPanel } from './hooks/useRightPanel';
import ThreadPanel from '@/features/threads/ThreadPanel';
import SearchPanel from '@/features/search/SearchPanel';
import PinnedMessagesPanel from '@/features/channel/PinnedMessagesPanel';
import ChannelFilesPanel from '@/features/channel/ChannelFilesPanel';
import ChannelMembersPanel from '@/features/channel/ChannelMembersPanel';
import NotificationsPanel from '@/features/notifications/NotificationsPanel';
import { useCurrentWorkspaceRole } from './hooks/useCurrentWorkspaceRole';
import ScheduledMessagesPanel from '@/features/messaging/ScheduledMessagesPanel';
import LaterPanel from '@/features/later/LaterPanel';
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
  onNavigateToMessage: (channelId: string, messageId: string, withThread?: boolean) => void;
  onOpenConversation: (conversationId: string) => void;
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
  onNavigateToMessage,
  onOpenConversation,
}: Props) {
  const { role } = useCurrentWorkspaceRole();

  if (!panel) return null;

  if (panel.kind === 'thread') {
    return (
      <ThreadPanel
        parentMessage={panel.message}
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
        onNavigateToMessage={(chId, msgId) => onNavigateToMessage(chId, msgId)}
        onOpenConversation={onOpenConversation}
      />
    );
  }
  if (panel.kind === 'search' && currentWorkspace) {
    return (
      <SearchPanel
        members={workspaceMembers}
        onClose={onClose}
        onNavigateToMessage={(chId, msgId) => onNavigateToMessage(chId, msgId)}
        onNavigateToConversation={onOpenConversation}
      />
    );
  }
  if (panel.kind === 'pins' && currentChannel) {
    return (
      <PinnedMessagesPanel
        channelId={currentChannel.id}
        onClose={onClose}
        onNavigate={(msgId) => onNavigateToMessage(currentChannel.id, msgId)}
      />
    );
  }
  if (panel.kind === 'files' && currentChannel) {
    return (
      <ChannelFilesPanel
        channelId={currentChannel.id}
        onClose={onClose}
        onNavigate={(msgId) => onNavigateToMessage(currentChannel.id, msgId)}
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
      <NotificationsPanel
        workspaceId={currentWorkspace.id}
        onClose={onClose}
        onNavigate={(chId, msgId, withThread) => onNavigateToMessage(chId, msgId, withThread)}
      />
    );
  }
  if (panel.kind === 'workflows' && currentWorkspace && currentUserId) {
    return (
      <WorkflowsPanel
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
