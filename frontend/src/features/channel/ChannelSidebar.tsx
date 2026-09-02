import { useState } from 'react';
import type { CreateChannelDraft } from '@/models/channel';
import { useNavigate } from 'react-router';
import { Compass } from 'lucide-react';
import type { Channel, Workspace, WorkspaceMember } from '@/stores/workspace';
import type { Conversation } from '@/hooks/queries/useConversations';
import { useCurrentWorkspaceRole } from '@/features/workspace/hooks/useCurrentWorkspaceRole';
import { useInstanceStore } from '@/stores/instances';
import { useUnreadNotificationCount } from '@/hooks/queries/useNotifications';
import { useMyStatus } from '@/hooks/queries/useStatus';
import BrowseChannelsModal from './BrowseChannelsModal';
import { useCollapsedSections } from './hooks/useCollapsedSections';
import { SidebarSectionHeader } from './components/SidebarSectionHeader';
import { SidebarChannelRow } from './components/SidebarChannelRow';
import { ConversationRow, MemberRow } from './components/SidebarPeople';
import { WorkspaceMenu } from './components/WorkspaceMenu';
import { YouMenu } from './components/YouMenu';
import { NewDmModal } from './components/NewDmModal';
import { CreateChannelModal } from './components/CreateChannelModal';

interface Props {
  currentWorkspace: Workspace | null;
  channels: Channel[];
  currentChannel: Channel | null;
  unreadChannels: Set<string>;
  mentionChannels: Set<string>;
  unreadCounts: Record<string, number>;
  mentionCounts: Record<string, number>;
  mutedChannels: Set<string>;
  workspaceMembers: WorkspaceMember[];
  currentUserId: string | undefined;
  user: { id: string; display_name: string; email: string; avatar_url: string | null } | null;
  conversations: Conversation[];
  currentConversationId: string | null;
  unreadConversations: Set<string>;
  onSelectChannel: (ch: Channel) => void;
  onCreateChannel: (draft: CreateChannelDraft) => Promise<void>;
  onToggleMute: (channelId: string, muted: boolean) => void;
  onOpenConversation: (conversationId: string) => void;
  onOpenWith: (participantIds: string[]) => Promise<void>;
  onOpenMembers: () => void;
  onOpenSettings: () => void;
  onOpenIntegrations: () => void;
  onOpenCustomEmoji: () => void;
  onOpenUserGroups: () => void;
  onOpenAuditLog: () => void;
  onOpenScheduled: () => void;
  onOpenSaved: () => void;
  onOpenSlackImport: () => void;
  onOpenReminders: () => void;
  onOpenProfile: () => void;
  onOpenNotifications: () => void;
  onLogout: () => void;
}

export default function ChannelSidebar({
  currentWorkspace,
  channels,
  currentChannel,
  unreadChannels,
  mentionChannels,
  unreadCounts,
  mentionCounts,
  mutedChannels,
  workspaceMembers,
  currentUserId,
  user,
  conversations,
  currentConversationId,
  unreadConversations,
  onSelectChannel,
  onCreateChannel,
  onToggleMute,
  onOpenConversation,
  onOpenWith,
  onOpenMembers,
  onOpenSettings,
  onOpenIntegrations,
  onOpenCustomEmoji,
  onOpenUserGroups,
  onOpenAuditLog,
  onOpenScheduled,
  onOpenSaved,
  onOpenSlackImport,
  onOpenReminders,
  onOpenProfile,
  onOpenNotifications,
  onLogout,
}: Props) {
  const { data: unreadNotifCount = 0 } = useUnreadNotificationCount(currentWorkspace?.id ?? null);
  const { data: myStatus } = useMyStatus(currentWorkspace?.instanceUrl);
  const { role: currentUserRole } = useCurrentWorkspaceRole();
  const navigate = useNavigate();
  const { instances, activeInstanceUrl } = useInstanceStore();
  const currentInstance = instances.find((i) => i.url === activeInstanceUrl);
  const isInstanceAdmin = currentInstance?.user.is_instance_admin ?? false;
  const isWorkspaceAdmin = currentUserRole === 'admin' || currentUserRole === 'owner';

  const { collapsed, toggleSection } = useCollapsedSections();
  const [showNewChannel, setShowNewChannel] = useState(false);
  const [showDmPicker, setShowDmPicker] = useState(false);
  const [showBrowseChannels, setShowBrowseChannels] = useState(false);

  return (
    <>
      <div
        role="navigation"
        aria-label="Channels and direct messages"
        // Opaque below `lg`: at that width this is a drawer floating over the
        // message list, and a translucent panel let the messages read through
        // the channel names.
        className="w-60 bg-surface lg:bg-surface/50 flex flex-col border-r border-line/50"
      >
        <WorkspaceMenu
          workspaceName={currentWorkspace?.name}
          isWorkspaceAdmin={isWorkspaceAdmin}
          isInstanceAdmin={isInstanceAdmin}
          onOpenMembers={onOpenMembers}
          onOpenSettings={onOpenSettings}
          onOpenIntegrations={onOpenIntegrations}
          onOpenSlackImport={onOpenSlackImport}
          onOpenCustomEmoji={onOpenCustomEmoji}
          onOpenUserGroups={onOpenUserGroups}
          onOpenAuditLog={onOpenAuditLog}
          onOpenInstanceAdmin={() => navigate('/app/admin')}
        />

        <div className="flex-1 overflow-y-auto py-2">
          <SidebarSectionHeader
            section="channels"
            label="Channels"
            collapsed={collapsed.channels}
            onToggle={toggleSection}
            action={{ label: 'Create channel', onClick: () => setShowNewChannel(true) }}
          />
          {!collapsed.channels &&
            channels.map((ch) => (
              <SidebarChannelRow
                key={ch.id}
                channel={ch}
                active={currentChannel?.id === ch.id}
                muted={mutedChannels.has(ch.id)}
                unread={unreadChannels.has(ch.id)}
                mentioned={mentionChannels.has(ch.id)}
                unreadCount={unreadCounts[ch.id]}
                mentionCount={mentionCounts[ch.id]}
                onSelect={onSelectChannel}
                onToggleMute={onToggleMute}
              />
            ))}
          {!collapsed.channels && currentUserRole !== 'guest' && (
            <button
              onClick={() => setShowBrowseChannels(true)}
              data-qa="browse-channels-open"
              className="w-full px-3 py-1.5 flex items-center gap-2 text-sm text-muted hover:bg-raised/30 hover:text-fg-soft transition cursor-pointer"
            >
              <Compass className="w-4 h-4 shrink-0" />
              <span className="truncate">Browse channels</span>
            </button>
          )}

          <SidebarSectionHeader
            section="dms"
            label="Direct Messages"
            collapsed={collapsed.dms}
            onToggle={toggleSection}
            action={{ label: 'New direct message', onClick: () => setShowDmPicker(true) }}
            className="mt-4"
          />
          {collapsed.dms ? null : conversations.length === 0 ? (
            <div className="px-3 py-1.5 text-xs text-muted">No conversations yet</div>
          ) : (
            conversations.map((conv) => (
              <ConversationRow
                key={conv.id}
                conversation={conv}
                currentUserId={currentUserId}
                isActive={currentConversationId === conv.id}
                isUnread={unreadConversations.has(conv.id)}
                onSelect={onOpenConversation}
              />
            ))
          )}

          {workspaceMembers.length > 0 && (
            <>
              <SidebarSectionHeader
                section="people"
                label="People"
                collapsed={collapsed.people}
                onToggle={toggleSection}
                className="mt-4"
              />
              {!collapsed.people &&
                workspaceMembers
                  .filter((m) => m.user_id !== currentUserId)
                  .map((m) => (
                    <MemberRow key={m.user_id} userId={m.user_id} onOpenDm={(id) => void onOpenWith([id])} />
                  ))}
            </>
          )}
        </div>

        <YouMenu
          user={user}
          statusEmoji={myStatus?.status_emoji}
          statusText={myStatus?.status_text}
          unreadNotifCount={unreadNotifCount}
          onOpenProfile={onOpenProfile}
          onOpenScheduled={onOpenScheduled}
          onOpenSaved={onOpenSaved}
          onOpenReminders={onOpenReminders}
          onOpenNotifications={onOpenNotifications}
          onLogout={onLogout}
        />
      </div>

      {showDmPicker && (
        <NewDmModal
          members={workspaceMembers}
          currentUserId={currentUserId}
          onStart={(people) => void onOpenWith(people)}
          onClose={() => setShowDmPicker(false)}
        />
      )}

      {showNewChannel && (
        <CreateChannelModal
          channels={channels}
          onCreate={onCreateChannel}
          onClose={() => setShowNewChannel(false)}
        />
      )}

      {showBrowseChannels && currentWorkspace && (
        <BrowseChannelsModal
          workspaceId={currentWorkspace.id}
          instanceUrl={currentWorkspace.instanceUrl}
          currentUserId={currentUserId}
          onClose={() => setShowBrowseChannels(false)}
          onOpenChannel={(channelId) => navigate(`/app/${currentWorkspace.id}/${channelId}`)}
        />
      )}
    </>
  );
}
