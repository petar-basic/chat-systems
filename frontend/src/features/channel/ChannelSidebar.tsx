import { useState } from 'react';
import type { CreateChannelDraft } from '@/models/channel';
import { useNavigate } from 'react-router';
import { Bookmark, Compass, FolderPlus, Inbox, MessagesSquare } from 'lucide-react';
import { useLater } from '@/hooks/queries/useLater';
import { useUnreadThreadCount } from '@/hooks/queries/useThreads';
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
import { SidebarCustomSection } from './components/SidebarCustomSection';
import { acceptsChannelDrop } from './components/channelDrag';
import { SectionNameInput } from './components/SectionNameInput';
import { useSidebarLayout, useSidebarActions } from '@/hooks/queries/useSidebarLayout';

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
  onOpenApps: () => void;
  onOpenCustomEmoji: () => void;
  onOpenUserGroups: () => void;
  onOpenAuditLog: () => void;
  onOpenScheduled: () => void;
  onOpenLater: () => void;
  laterActive: boolean;
  onOpenSlackImport: () => void;
  onOpenProfile: () => void;
  onOpenNotifications: () => void;
  onOpenUnreads: () => void;
  unreadsActive: boolean;
  onOpenThreads: () => void;
  threadsActive: boolean;
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
  onOpenApps,
  onOpenCustomEmoji,
  onOpenUserGroups,
  onOpenAuditLog,
  onOpenScheduled,
  onOpenLater,
  laterActive,
  onOpenSlackImport,
  onOpenProfile,
  onOpenNotifications,
  onOpenUnreads,
  unreadsActive,
  onOpenThreads,
  threadsActive,
  onLogout,
}: Props) {
  const { data: unreadNotifCount = 0 } = useUnreadNotificationCount(currentWorkspace?.id ?? null);
  const { data: myStatus } = useMyStatus(currentWorkspace?.instanceUrl);
  const { data: laterItems } = useLater(currentWorkspace?.id, 'in_progress', currentWorkspace?.instanceUrl);
  const laterCount = laterItems?.length ?? 0;
  const { data: unreadThreads = 0 } = useUnreadThreadCount(currentWorkspace?.id);
  const { role: currentUserRole } = useCurrentWorkspaceRole();
  const navigate = useNavigate();
  const { instances, activeInstanceUrl } = useInstanceStore();
  const currentInstance = instances.find((i) => i.url === activeInstanceUrl);
  const isInstanceAdmin = currentInstance?.user.is_instance_admin ?? false;
  const isWorkspaceAdmin = currentUserRole === 'admin' || currentUserRole === 'owner';

  const { data: layout } = useSidebarLayout(currentWorkspace?.id);
  const sidebar = useSidebarActions(currentWorkspace?.id);
  const { collapsed, toggleSection } = useCollapsedSections(layout, (section, isCollapsed) =>
    sidebar.collapseBuiltin.mutate({ section, collapsed: isCollapsed }),
  );
  const [creatingSection, setCreatingSection] = useState(false);
  const sections = [...(layout?.sections ?? [])].sort((a, b) => a.position - b.position);
  const sectionIds = new Set(sections.map((section) => section.id));
  const placement = new Map(
    (layout?.placements ?? [])
      .filter((p) => sectionIds.has(p.section_id))
      .map((p) => [p.channel_id, p.section_id] as const),
  );
  const moveTargets = [
    { id: null, name: 'Channels' },
    ...sections.map((section) => ({ id: section.id, name: section.name })),
  ];
  const moveChannel = (channelId: string, sectionId: string | null) =>
    sidebar.placeChannel.mutate({ channelId, sectionId });
  const reorder = (from: number, to: number) => {
    const ids = sections.map((section) => section.id);
    [ids[from], ids[to]] = [ids[to], ids[from]];
    sidebar.reorderSections.mutate(ids);
  };
  const renderChannel = (ch: Channel) => (
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
      moveTargets={moveTargets}
      sectionId={placement.get(ch.id) ?? null}
      onMove={moveChannel}
    />
  );
  const [showNewChannel, setShowNewChannel] = useState(false);
  const totalUnread =
    channels.reduce((sum, ch) => sum + (mutedChannels.has(ch.id) ? 0 : (unreadCounts[ch.id] ?? 0)), 0) +
    unreadConversations.size;
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
          onOpenApps={onOpenApps}
          onOpenSlackImport={onOpenSlackImport}
          onOpenCustomEmoji={onOpenCustomEmoji}
          onOpenUserGroups={onOpenUserGroups}
          onOpenAuditLog={onOpenAuditLog}
          onOpenInstanceAdmin={() => navigate('/app/admin')}
        />

        <div className="flex-1 overflow-y-auto py-2">
          <button
            type="button"
            onClick={onOpenUnreads}
            aria-current={unreadsActive ? 'page' : undefined}
            title="All unreads (Shift+Alt+A)"
            data-qa="all-unreads-open"
            className={`w-full px-3 py-1.5 mb-2 flex items-center gap-2 text-sm transition cursor-pointer ${
              unreadsActive ? 'bg-purple-600/20 text-fg' : 'text-fg-dim hover:bg-raised/30 hover:text-fg-soft'
            } ${totalUnread > 0 ? 'font-semibold' : ''}`}
          >
            <Inbox className="w-4 h-4 shrink-0" />
            <span className="truncate">All unreads</span>
            {totalUnread > 0 && (
              <span className="ml-auto text-xs px-1.5 rounded-full bg-raised text-fg-soft">
                {totalUnread > 99 ? '99+' : totalUnread}
              </span>
            )}
          </button>
          <button
            type="button"
            onClick={onOpenThreads}
            aria-current={threadsActive ? 'page' : undefined}
            data-qa="threads-open"
            className={`w-full px-3 py-1.5 mb-2 -mt-1.5 flex items-center gap-2 text-sm transition cursor-pointer ${
              threadsActive ? 'bg-purple-600/20 text-fg' : 'text-fg-dim hover:bg-raised/30 hover:text-fg-soft'
            } ${unreadThreads > 0 ? 'font-semibold' : ''}`}
          >
            <MessagesSquare className="w-4 h-4 shrink-0" />
            <span className="truncate">Threads</span>
            {unreadThreads > 0 && (
              <span
                className="ml-auto text-xs px-1.5 rounded-full bg-raised text-fg-soft"
                data-qa="threads-count"
              >
                {unreadThreads > 99 ? '99+' : unreadThreads}
              </span>
            )}
          </button>
          <button
            type="button"
            onClick={onOpenLater}
            aria-pressed={laterActive}
            data-qa="later-open"
            className={`w-full px-3 py-1.5 mb-2 -mt-1.5 flex items-center gap-2 text-sm transition cursor-pointer ${
              laterActive ? 'bg-purple-600/20 text-fg' : 'text-fg-dim hover:bg-raised/30 hover:text-fg-soft'
            }`}
          >
            <Bookmark className="w-4 h-4 shrink-0" />
            <span className="truncate">Later</span>
            {laterCount > 0 && (
              <span
                className="ml-auto text-xs px-1.5 rounded-full bg-raised text-fg-soft"
                data-qa="later-count"
              >
                {laterCount > 99 ? '99+' : laterCount}
              </span>
            )}
          </button>
          {sections.map((section, index) => (
            <SidebarCustomSection
              key={section.id}
              id={section.id}
              name={section.name}
              collapsed={section.collapsed}
              onToggle={() => sidebar.updateSection.mutate({ id: section.id, collapsed: !section.collapsed })}
              onRename={(name) => sidebar.updateSection.mutate({ id: section.id, name })}
              onDelete={() => sidebar.deleteSection.mutate(section.id)}
              onMoveUp={index > 0 ? () => reorder(index, index - 1) : undefined}
              onMoveDown={index < sections.length - 1 ? () => reorder(index, index + 1) : undefined}
              onDropChannel={(channelId) => moveChannel(channelId, section.id)}
            >
              {channels.filter((ch) => placement.get(ch.id) === section.id).map(renderChannel)}
            </SidebarCustomSection>
          ))}

          <div
            className={sections.length > 0 ? 'mt-4' : ''}
            {...acceptsChannelDrop((id) => moveChannel(id, null))}
          >
            <SidebarSectionHeader
              section="channels"
              label="Channels"
              collapsed={collapsed.channels}
              onToggle={toggleSection}
              action={{ label: 'Create channel', onClick: () => setShowNewChannel(true) }}
            />
            {!collapsed.channels && channels.filter((ch) => !placement.has(ch.id)).map(renderChannel)}
          </div>
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
          {!collapsed.channels &&
            (creatingSection ? (
              <div className="px-3 py-1">
                <SectionNameInput
                  label="New section name"
                  onSubmit={(name) => {
                    setCreatingSection(false);
                    sidebar.createSection.mutate(name);
                  }}
                  onCancel={() => setCreatingSection(false)}
                />
              </div>
            ) : (
              <button
                onClick={() => setCreatingSection(true)}
                data-qa="sidebar-section-create"
                className="w-full px-3 py-1.5 flex items-center gap-2 text-sm text-muted hover:bg-raised/30 hover:text-fg-soft transition cursor-pointer"
              >
                <FolderPlus className="w-4 h-4 shrink-0" />
                <span className="truncate">Create section</span>
              </button>
            ))}

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
          onOpenLater={onOpenLater}
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
          onCreateChannel={() => {
            setShowBrowseChannels(false);
            setShowNewChannel(true);
          }}
        />
      )}
    </>
  );
}
