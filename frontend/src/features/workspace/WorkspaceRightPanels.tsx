import type { Channel, WorkspaceMember, Workspace } from '@/stores/workspace';
import type { Conversation } from '@/hooks/queries/useConversations';
import type { RightPanel } from './hooks/useRightPanel';
import MembersPanel from '@/components/MembersPanel';
import SettingsPanel from '@/components/SettingsPanel';
import ThreadPanel from '@/components/ThreadPanel';
import SearchPanel from '@/components/SearchPanel';
import PinnedMessagesPanel from '@/components/PinnedMessagesPanel';
import ChannelMembersPanel from '@/components/ChannelMembersPanel';
import NotificationsPanel from '@/components/NotificationsPanel';
import IntegrationsPanel from '@/components/IntegrationsPanel';
import CustomEmojiPanel from '@/components/CustomEmojiPanel';
import UserGroupsPanel from '@/components/UserGroupsPanel';
import { useCurrentWorkspaceRole } from './hooks/useCurrentWorkspaceRole';
import AuditLogPanel from '@/components/AuditLogPanel';
import ScheduledMessagesPanel from '@/components/ScheduledMessagesPanel';

interface Props {
  panel: RightPanel;
  currentWorkspace: Workspace | null;
  currentChannel: Channel | null;
  workspaceMembers: WorkspaceMember[];
  channels: Channel[];
  conversations: Conversation[];
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
  onClose,
  onNavigateToMessage,
  onOpenConversation,
}: Props) {
  const { role } = useCurrentWorkspaceRole();

  if (!panel) return null;

  if (panel.kind === 'members' && currentWorkspace) {
    return <MembersPanel workspaceId={currentWorkspace.id} onClose={onClose} />;
  }
  if (panel.kind === 'settings' && currentWorkspace) {
    return (
      <SettingsPanel
        workspaceId={currentWorkspace.id}
        instanceUrl={currentWorkspace.instanceUrl}
        currentName={currentWorkspace.name}
        currentDescription={currentWorkspace.description}
        deletedAt={currentWorkspace.deleted_at}
        onClose={onClose}
      />
    );
  }
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
  if (panel.kind === 'search' && currentWorkspace) {
    return (
      <SearchPanel
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
  if (panel.kind === 'channelMembers' && currentChannel && currentWorkspace) {
    return (
      <ChannelMembersPanel
        channelId={currentChannel.id}
        channelName={currentChannel.name}
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
  if (panel.kind === 'integrations' && currentWorkspace) {
    return (
      <IntegrationsPanel
        workspaceId={currentWorkspace.id}
        instanceUrl={currentWorkspace.instanceUrl}
        channels={channels}
        onClose={onClose}
      />
    );
  }
  if (panel.kind === 'customEmoji' && currentWorkspace) {
    return (
      <CustomEmojiPanel
        workspaceId={currentWorkspace.id}
        instanceUrl={currentWorkspace.instanceUrl}
        onClose={onClose}
      />
    );
  }
  if (panel.kind === 'userGroups' && currentWorkspace) {
    return (
      <UserGroupsPanel
        workspaceId={currentWorkspace.id}
        instanceUrl={currentWorkspace.instanceUrl}
        members={workspaceMembers}
        isAdmin={role === 'admin' || role === 'owner'}
        onClose={onClose}
      />
    );
  }
  if (panel.kind === 'auditLog' && currentWorkspace) {
    return (
      <AuditLogPanel
        workspaceId={currentWorkspace.id}
        instanceUrl={currentWorkspace.instanceUrl}
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
