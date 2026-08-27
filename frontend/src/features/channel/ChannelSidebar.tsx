import { useRef, useState, type FormEvent } from 'react';
import { useNavigate } from 'react-router';
import { useOnClickOutside } from '@/shared/hooks/useOnClickOutside';
import { useEscapeToClose } from '@/shared/hooks/useEscapeToClose';
import { Modal } from '@/shared/components/Modal/Modal';
import {
  Hash,
  Lock,
  Plus,
  ChevronDown,
  Users,
  Settings,
  LogOut,
  ShieldCheck,
  Bell,
  BellOff,
  Compass,
  Plug,
  ScrollText,
  Check,
  Clock,
  Smile,
  Bookmark,
  BellRing,
  Upload,
  User,
} from 'lucide-react';
import type { Channel, Workspace, WorkspaceMember } from '@/stores/workspace';
import type { Conversation } from '@/hooks/queries/useConversations';
import { conversationTitle } from '@/lib/conversationHelpers';
import { useUserCache } from '@/stores/users';
import { useCurrentWorkspaceRole } from '@/features/workspace/hooks/useCurrentWorkspaceRole';
import { useInstanceStore } from '@/stores/instances';
import { useUnreadNotificationCount } from '@/hooks/queries/useNotifications';
import { displayNameOf } from '@/lib/userHelpers';
import { useMyStatus } from '@/hooks/queries/useStatus';
import BrowseChannelsModal from './BrowseChannelsModal';
import { Avatar } from '@/shared/components/Avatar/Avatar';
import PresenceDot from '@/components/PresenceDot';

const MAX_GROUP_OTHERS = 8;

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
  onCreateChannel: (name: string) => Promise<void>;
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

/** Slack stops at 99+, and so does the space in the sidebar. */
function formatBadge(count: number): string {
  return count > 99 ? '99+' : String(count);
}

function UserAvatarWithPresence({
  userId,
  name,
  avatarUrl,
}: {
  userId: string;
  name: string;
  avatarUrl: string | null | undefined;
}) {
  return (
    <div className="relative shrink-0">
      <Avatar userId={userId} name={name} avatarUrl={avatarUrl} size="xs" />
      <PresenceDot
        userId={userId}
        className="absolute -bottom-0.5 -right-0.5 w-2 h-2 ring-2 ring-slate-800"
      />
    </div>
  );
}

function ConversationButton({
  conversation,
  currentUserId,
  isActive,
  isUnread,
  onSelect,
}: {
  conversation: Conversation;
  currentUserId: string | undefined;
  isActive: boolean;
  isUnread: boolean;
  onSelect: (conversationId: string) => void;
}) {
  const { getUser } = useUserCache();
  const title = conversationTitle(conversation, currentUserId, (id) => getUser(id)?.display_name);
  const others = conversation.participant_ids.filter((id) => id !== currentUserId);
  const partnerId = others[0] ?? currentUserId ?? '';

  return (
    <button
      onClick={() => onSelect(conversation.id)}
      data-qa="conversation-row"
      data-conversation-id={conversation.id}
      className={`w-full px-3 py-1.5 flex items-center gap-2 text-sm transition cursor-pointer ${
        isActive
          ? 'bg-purple-600/20 text-white'
          : isUnread
            ? 'text-white font-semibold hover:bg-slate-700/30'
            : 'text-slate-400 hover:bg-slate-700/30 hover:text-slate-200'
      }`}
    >
      {conversation.kind === 'group' ? (
        <span className="relative shrink-0 flex -space-x-2">
          {others.slice(0, 2).map((id) => (
            <Avatar
              key={id}
              userId={id}
              name={displayNameOf(getUser(id)?.display_name)}
              avatarUrl={getUser(id)?.avatar_url}
              size="xs"
              className="ring-2 ring-slate-800"
            />
          ))}
        </span>
      ) : (
        <UserAvatarWithPresence userId={partnerId} name={title} avatarUrl={getUser(partnerId)?.avatar_url} />
      )}
      <span className="truncate">{title}</span>
      {isUnread && !isActive && <span className="ml-auto w-2 h-2 bg-purple-400 rounded-full shrink-0" />}
    </button>
  );
}

function SidebarUser({ userId, onOpenDm }: { userId: string; onOpenDm: (id: string) => void }) {
  const { getUser } = useUserCache();
  const cached = getUser(userId);

  const name = displayNameOf(cached?.display_name);

  return (
    <button
      onClick={() => onOpenDm(userId)}
      className="w-full px-3 py-1 flex items-center gap-2 text-sm text-slate-400 hover:bg-slate-700/30 hover:text-slate-200 transition cursor-pointer"
      title={cached?.status_text ? `Message ${name} — ${cached.status_text}` : `Message ${name}`}
    >
      <UserAvatarWithPresence userId={userId} name={name} avatarUrl={cached?.avatar_url} />
      <span className="truncate">{name}</span>
      {cached?.status_emoji && (
        <span className="shrink-0" data-qa="member-status-emoji" title={cached.status_text ?? undefined}>
          {cached.status_emoji}
        </span>
      )}
    </button>
  );
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

  const [wsDropdownOpen, setWsDropdownOpen] = useState(false);
  const [youMenuOpen, setYouMenuOpen] = useState(false);
  const youRef = useRef<HTMLDivElement>(null);
  useOnClickOutside(youRef, () => setYouMenuOpen(false), youMenuOpen);
  const [showNewChannel, setShowNewChannel] = useState(false);
  const [newChannelName, setNewChannelName] = useState('');
  const [showDmPicker, setShowDmPicker] = useState(false);
  const [dmSearch, setDmSearch] = useState('');
  const [showBrowseChannels, setShowBrowseChannels] = useState(false);

  const [selectedPeople, setSelectedPeople] = useState<string[]>([]);

  const closeDmPicker = () => {
    setShowDmPicker(false);
    setDmSearch('');
    setSelectedPeople([]);
  };

  const togglePerson = (userId: string) =>
    setSelectedPeople((picked) =>
      picked.includes(userId)
        ? picked.filter((id) => id !== userId)
        : picked.length >= MAX_GROUP_OTHERS
          ? picked
          : [...picked, userId],
    );

  const dmQuery = dmSearch.trim().toLowerCase();
  const dmCandidates = workspaceMembers.filter((m) =>
    (m.display_name || m.email).toLowerCase().includes(dmQuery),
  );

  const wsDropdownRef = useRef<HTMLDivElement>(null);
  useOnClickOutside(wsDropdownRef, () => setWsDropdownOpen(false), wsDropdownOpen);
  useEscapeToClose(() => setWsDropdownOpen(false), wsDropdownOpen);

  const handleCreateChannel = async (e: FormEvent) => {
    e.preventDefault();
    if (!newChannelName.trim()) return;
    await onCreateChannel(newChannelName.trim());
    setNewChannelName('');
    setShowNewChannel(false);
  };

  const channelIcon = (ch: Channel) =>
    ch.channel_type === 'private' ? (
      <Lock className="w-4 h-4 text-slate-400 shrink-0" />
    ) : (
      <Hash className="w-4 h-4 text-slate-400 shrink-0" />
    );

  const channelButton = (ch: Channel, icon: React.ReactNode) => {
    const muted = mutedChannels.has(ch.id);
    const active = currentChannel?.id === ch.id;
    const unread = unreadChannels.has(ch.id) && !muted;
    return (
      <div key={ch.id} className="group relative flex items-center">
        <button
          onClick={() => onSelectChannel(ch)}
          className={`flex-1 min-w-0 px-3 py-1.5 flex items-center gap-2 text-sm transition ${
            active
              ? 'bg-purple-600/20 text-white'
              : unread
                ? 'text-white font-semibold hover:bg-slate-700/30'
                : muted
                  ? 'text-slate-500 hover:bg-slate-700/30'
                  : 'text-slate-400 hover:bg-slate-700/30 hover:text-slate-200'
          }`}
        >
          {icon}
          <span className="truncate">{ch.name || 'Channel'}</span>
          {muted && <BellOff className="w-3 h-3 text-slate-600 ml-auto shrink-0" />}
          {!muted && mentionChannels.has(ch.id) ? (
            <span
              aria-hidden="true"
              data-qa="channel-mention-badge"
              data-channel-id={ch.id}
              className="ml-auto min-w-5 h-5 px-1 bg-red-500 rounded-full shrink-0 flex items-center justify-center text-[10px] font-bold text-white"
            >
              {mentionCounts[ch.id] ? formatBadge(mentionCounts[ch.id]) : '@'}
            </span>
          ) : (
            !muted &&
            unread &&
            !!unreadCounts[ch.id] && (
              <span
                aria-hidden="true"
                data-qa="channel-unread-badge"
                data-channel-id={ch.id}
                className="ml-auto min-w-5 h-5 px-1 bg-slate-600 rounded-full shrink-0 flex items-center justify-center text-[10px] font-bold text-white"
              >
                {formatBadge(unreadCounts[ch.id])}
              </span>
            )
          )}
        </button>
        <button
          onClick={() => onToggleMute(ch.id, !muted)}
          aria-label={muted ? `Unmute ${ch.name}` : `Mute ${ch.name}`}
          title={muted ? 'Unmute' : 'Mute'}
          className="absolute right-1 hidden group-hover:flex p-1 rounded text-slate-400 hover:text-white hover:bg-slate-700"
        >
          {muted ? <Bell className="w-3 h-3" /> : <BellOff className="w-3 h-3" />}
        </button>
      </div>
    );
  };

  return (
    <>
      <div
        role="navigation"
        aria-label="Channels and direct messages"
        // Opaque below `lg`: at that width this is a drawer floating over the
        // message list, and a translucent panel let the messages read through
        // the channel names.
        className="w-60 bg-slate-800 lg:bg-slate-800/50 flex flex-col border-r border-slate-700/50"
      >
        <div className="relative" ref={wsDropdownRef}>
          <button
            onClick={() => setWsDropdownOpen(!wsDropdownOpen)}
            className="w-full px-4 py-3 flex items-center justify-between border-b border-slate-700/50 hover:bg-slate-700/30 transition cursor-pointer"
          >
            <span className="font-semibold text-white truncate">
              {currentWorkspace?.name || 'Select workspace'}
            </span>
            <ChevronDown className="w-4 h-4 text-slate-400" />
          </button>
          {wsDropdownOpen && (
            <div className="absolute top-full left-0 right-0 bg-slate-800 border border-slate-700 rounded-b-lg shadow-xl z-10">
              <button
                className="w-full px-4 py-2 text-left text-sm text-slate-300 hover:bg-slate-700 flex items-center gap-2 cursor-pointer"
                onClick={() => {
                  onOpenMembers();
                  setWsDropdownOpen(false);
                }}
              >
                <Users className="w-4 h-4" /> Members
              </button>
              <button
                className="w-full px-4 py-2 text-left text-sm text-slate-300 hover:bg-slate-700 flex items-center gap-2 cursor-pointer"
                onClick={() => {
                  onOpenSettings();
                  setWsDropdownOpen(false);
                }}
              >
                <Settings className="w-4 h-4" /> Settings
              </button>
              {isWorkspaceAdmin && (
                <button
                  className="w-full px-4 py-2 text-left text-sm text-slate-300 hover:bg-slate-700 flex items-center gap-2 cursor-pointer"
                  data-qa="open-integrations"
                  onClick={() => {
                    onOpenIntegrations();
                    setWsDropdownOpen(false);
                  }}
                >
                  <Plug className="w-4 h-4" /> Integrations
                </button>
              )}
              {isWorkspaceAdmin && (
                <button
                  className="w-full px-4 py-2 text-left text-sm text-slate-300 hover:bg-slate-700 flex items-center gap-2 cursor-pointer"
                  data-qa="open-slack-import"
                  onClick={() => {
                    onOpenSlackImport();
                    setWsDropdownOpen(false);
                  }}
                >
                  <Upload className="w-4 h-4" /> Slack import
                </button>
              )}
              <button
                className="w-full px-4 py-2 text-left text-sm text-slate-300 hover:bg-slate-700 flex items-center gap-2 cursor-pointer"
                data-qa="open-custom-emoji"
                onClick={() => {
                  onOpenCustomEmoji();
                  setWsDropdownOpen(false);
                }}
              >
                <Smile className="w-4 h-4" /> Custom emoji
              </button>
              <button
                className="w-full px-4 py-2 text-left text-sm text-slate-300 hover:bg-slate-700 flex items-center gap-2 cursor-pointer"
                data-qa="open-user-groups"
                onClick={() => {
                  onOpenUserGroups();
                  setWsDropdownOpen(false);
                }}
              >
                <Users className="w-4 h-4" /> User groups
              </button>
              {isWorkspaceAdmin && (
                <button
                  className="w-full px-4 py-2 text-left text-sm text-slate-300 hover:bg-slate-700 flex items-center gap-2 cursor-pointer"
                  data-qa="open-audit-log"
                  onClick={() => {
                    onOpenAuditLog();
                    setWsDropdownOpen(false);
                  }}
                >
                  <ScrollText className="w-4 h-4" /> Audit log
                </button>
              )}
              {isInstanceAdmin && (
                <button
                  className="w-full px-4 py-2 text-left text-sm text-purple-300 hover:bg-slate-700 flex items-center gap-2 cursor-pointer border-t border-slate-700"
                  onClick={() => {
                    navigate('/app/admin');
                    setWsDropdownOpen(false);
                  }}
                >
                  <ShieldCheck className="w-4 h-4" /> Instance Admin
                </button>
              )}
            </div>
          )}
        </div>

        <div className="flex-1 overflow-y-auto py-2">
          <div className="px-3 mb-1 flex items-center justify-between">
            <span className="text-xs font-semibold text-slate-400 uppercase tracking-wider">Channels</span>
            <button
              onClick={() => setShowNewChannel(true)}
              aria-label="Create channel"
              title="Create channel"
              className="text-slate-400 hover:text-white transition cursor-pointer"
            >
              <Plus className="w-4 h-4" />
            </button>
          </div>
          {channels.map((ch) => channelButton(ch, channelIcon(ch)))}
          {currentUserRole !== 'guest' && (
            <button
              onClick={() => setShowBrowseChannels(true)}
              data-qa="browse-channels-open"
              className="w-full px-3 py-1.5 flex items-center gap-2 text-sm text-slate-400 hover:bg-slate-700/30 hover:text-slate-200 transition cursor-pointer"
            >
              <Compass className="w-4 h-4 shrink-0" />
              <span className="truncate">Browse channels</span>
            </button>
          )}

          <div className="px-3 mt-4 mb-1 flex items-center justify-between">
            <span className="text-xs font-semibold text-slate-400 uppercase tracking-wider">
              Direct Messages
            </span>
            <button
              onClick={() => setShowDmPicker(true)}
              className="text-slate-400 hover:text-white transition cursor-pointer"
              title="New direct message"
            >
              <Plus className="w-4 h-4" />
            </button>
          </div>
          {conversations.length === 0 ? (
            <div className="px-3 py-1.5 text-xs text-slate-400">No conversations yet</div>
          ) : (
            conversations.map((conv) => (
              <ConversationButton
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
              <div className="px-3 mt-4 mb-1">
                <span className="text-xs font-semibold text-slate-400 uppercase tracking-wider">People</span>
              </div>
              {workspaceMembers
                .filter((m) => m.user_id !== currentUserId)
                .map((m) => (
                  <SidebarUser key={m.user_id} userId={m.user_id} onOpenDm={(id) => void onOpenWith([id])} />
                ))}
            </>
          )}
        </div>

        <div className="px-3 py-3 border-t border-slate-700/50 flex items-center gap-2">
          <button
            onClick={onOpenProfile}
            aria-label="Edit profile"
            data-qa="sidebar-profile-avatar"
            className="rounded-full shrink-0 hover:ring-2 hover:ring-purple-400 transition cursor-pointer"
            title="Edit profile"
          >
            <Avatar
              userId={user?.id ?? ''}
              name={displayNameOf(user?.display_name)}
              avatarUrl={user?.avatar_url}
            />
          </button>
          <div className="relative flex-1 min-w-0" ref={youRef}>
            <button
              onClick={() => setYouMenuOpen((open) => !open)}
              data-qa="open-you-menu"
              className="w-full min-w-0 text-left hover:bg-slate-700/30 rounded px-1 -mx-1 transition cursor-pointer"
              title="You"
            >
              <div className="text-sm font-medium truncate">
                {user?.display_name}
                {myStatus?.status_emoji && (
                  <span className="ml-1.5" data-qa="own-status-emoji">
                    {myStatus.status_emoji}
                  </span>
                )}
              </div>
              <div className="text-xs text-slate-400 truncate">
                {myStatus?.status_text || user?.email}
              </div>
            </button>

            {/* Saved, scheduled and reminders follow the person, not the
                workspace: they are the same list whichever workspace is open,
                so they belong here rather than under a workspace's name. */}
            {youMenuOpen && (
              <div
                className="absolute bottom-full left-0 right-0 mb-1 bg-slate-800 border border-slate-700 rounded-lg shadow-xl z-20 py-1"
                data-qa="you-menu"
              >
                <button
                  className="w-full px-4 py-2 text-left text-sm text-slate-300 hover:bg-slate-700 flex items-center gap-2 cursor-pointer"
                  data-qa="open-profile"
                  onClick={() => {
                    onOpenProfile();
                    setYouMenuOpen(false);
                  }}
                >
                  <User className="w-4 h-4" /> Profile &amp; settings
                </button>
                <div className="my-1 h-px bg-slate-700" />
                <button
                  className="w-full px-4 py-2 text-left text-sm text-slate-300 hover:bg-slate-700 flex items-center gap-2 cursor-pointer"
                  data-qa="open-scheduled"
                  onClick={() => {
                    onOpenScheduled();
                    setYouMenuOpen(false);
                  }}
                >
                  <Clock className="w-4 h-4" /> Scheduled
                </button>
                <button
                  className="w-full px-4 py-2 text-left text-sm text-slate-300 hover:bg-slate-700 flex items-center gap-2 cursor-pointer"
                  data-qa="open-saved"
                  onClick={() => {
                    onOpenSaved();
                    setYouMenuOpen(false);
                  }}
                >
                  <Bookmark className="w-4 h-4" /> Saved
                </button>
                <button
                  className="w-full px-4 py-2 text-left text-sm text-slate-300 hover:bg-slate-700 flex items-center gap-2 cursor-pointer"
                  data-qa="open-reminders"
                  onClick={() => {
                    onOpenReminders();
                    setYouMenuOpen(false);
                  }}
                >
                  <BellRing className="w-4 h-4" /> Reminders
                </button>
              </div>
            )}
          </div>
          <button
            onClick={onOpenNotifications}
            className="relative text-slate-400 hover:text-white transition cursor-pointer"
            title="Notifications"
          >
            <Bell className="w-4 h-4" />
            {unreadNotifCount > 0 && (
              <span className="absolute -top-1.5 -right-1.5 min-w-[14px] h-3.5 px-0.5 bg-red-500 text-white text-[9px] font-bold rounded-full flex items-center justify-center leading-none">
                {unreadNotifCount > 99 ? '99+' : unreadNotifCount}
              </span>
            )}
          </button>
          <button
            onClick={onLogout}
            className="text-slate-400 hover:text-red-400 transition cursor-pointer"
            title="Sign out"
          >
            <LogOut className="w-4 h-4" />
          </button>
        </div>
      </div>

      {showDmPicker && (
        <Modal title="New Message" onClose={closeDmPicker} dataQa="new-dm-modal">
          <h2 className="text-lg font-bold mb-1">New message</h2>
          <p className="text-xs text-slate-400 mb-3">
            Pick one person for a direct message, or up to eight for a group.
          </p>
          <input
            type="text"
            value={dmSearch}
            onChange={(e) => setDmSearch(e.target.value)}
            placeholder="Search people…"
            aria-label="Search people"
            data-qa="new-dm-search"
            className="w-full px-3 py-2 mb-3 bg-slate-700/50 border border-slate-600 rounded-lg text-sm text-white placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-purple-500"
          />
          <div className="flex flex-col gap-1 max-h-64 overflow-y-auto">
            {dmCandidates.length === 0 ? (
              <div className="px-3 py-4 text-sm text-slate-400 text-center">No people found</div>
            ) : (
              dmCandidates
                .filter((m) => m.user_id !== currentUserId)
                .map((m) => {
                  const picked = selectedPeople.includes(m.user_id);
                  return (
                    <button
                      key={m.user_id}
                      onClick={() => togglePerson(m.user_id)}
                      aria-pressed={picked}
                      data-qa="new-dm-candidate"
                      data-user-id={m.user_id}
                      className={`w-full px-3 py-2 flex items-center gap-3 rounded-lg text-left transition ${
                        picked ? 'bg-purple-600/20 text-white' : 'hover:bg-slate-700/50'
                      }`}
                    >
                      <Avatar userId={m.user_id} name={m.display_name || m.email} avatarUrl={m.avatar_url} />
                      <div className="min-w-0">
                        <div className="text-sm font-medium truncate">{m.display_name || m.email}</div>
                        <div className="text-xs text-slate-400 truncate">{m.email}</div>
                      </div>
                      {picked && <Check className="w-4 h-4 text-purple-300 ml-auto shrink-0" />}
                    </button>
                  );
                })
            )}
          </div>
          <div className="mt-4 flex items-center justify-between gap-2">
            <span className="text-xs text-slate-400" data-qa="new-dm-selected-count">
              {selectedPeople.length === 0 ? 'Nobody picked yet' : `${selectedPeople.length} selected`}
            </span>
            <div className="flex gap-2">
              <button
                onClick={closeDmPicker}
                className="px-4 py-2 text-slate-400 hover:text-white transition"
              >
                Cancel
              </button>
              <button
                onClick={() => {
                  const people = selectedPeople;
                  closeDmPicker();
                  void onOpenWith(people);
                }}
                disabled={selectedPeople.length === 0}
                data-qa="new-dm-start"
                className="px-4 py-2 bg-purple-600 hover:bg-purple-500 disabled:bg-purple-600/50 text-white text-sm font-medium rounded-lg transition cursor-pointer disabled:cursor-not-allowed"
              >
                {selectedPeople.length > 1 ? 'Start group' : 'Start chat'}
              </button>
            </div>
          </div>
        </Modal>
      )}

      {showNewChannel && (
        <Modal title="Create Channel" onClose={() => setShowNewChannel(false)} dataQa="create-channel-modal">
          <form onSubmit={handleCreateChannel}>
            <h2 className="text-lg font-bold mb-4">Create Channel</h2>
            <input
              type="text"
              value={newChannelName}
              onChange={(e) => setNewChannelName(e.target.value)}
              placeholder="Channel name"
              className="w-full px-4 py-3 bg-slate-700/50 border border-slate-600 rounded-lg text-white placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-purple-500 mb-4"
              required
            />
            <div className="flex justify-end gap-2">
              <button
                type="button"
                onClick={() => setShowNewChannel(false)}
                className="px-4 py-2 text-slate-400 hover:text-white transition"
              >
                Cancel
              </button>
              <button
                type="submit"
                className="px-4 py-2 bg-purple-600 hover:bg-purple-500 text-white rounded-lg transition"
              >
                Create
              </button>
            </div>
          </form>
        </Modal>
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
