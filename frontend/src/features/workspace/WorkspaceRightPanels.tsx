import type { Channel, WorkspaceMember, Workspace } from '@/stores/workspace';
import type { RightPanel } from './hooks/useRightPanel';
import MembersPanel from '@/components/MembersPanel';
import SettingsPanel from '@/components/SettingsPanel';
import ThreadPanel from '@/components/ThreadPanel';
import SearchPanel from '@/components/SearchPanel';
import PinnedMessagesPanel from '@/components/PinnedMessagesPanel';
import ChannelMembersPanel from '@/components/ChannelMembersPanel';
import NotificationsPanel from '@/components/NotificationsPanel';

interface Props {
  panel: RightPanel;
  currentWorkspace: Workspace | null;
  currentChannel: Channel | null;
  workspaceMembers: WorkspaceMember[];
  channels: Channel[];
  onClose: () => void;
  onNavigateToMessage: (channelId: string, messageId: string, withThread?: boolean) => void;
}

export default function WorkspaceRightPanels({
  panel,
  currentWorkspace,
  currentChannel,
  workspaceMembers,
  channels,
  onClose,
  onNavigateToMessage,
}: Props) {
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
  return null;
}
