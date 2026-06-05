import { useRef, useState, type FormEvent } from 'react';
import { useNavigate } from 'react-router-dom';
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
  Circle,
  ShieldCheck,
  Bell,
  BellOff,
} from 'lucide-react';
import type { Channel, Workspace, WorkspaceMember } from '@/stores/workspace';
import type { DmConversation } from '@/hooks/queries/useDm';
import { useUserCache } from '@/stores/users';
import { usePresenceStore } from '@/stores/presence';
import { useInstanceStore } from '@/stores/instances';
import { useUnreadNotificationCount } from '@/hooks/queries/useNotifications';
import { displayNameOf } from '@/lib/userHelpers';

interface Props {
  currentWorkspace: Workspace | null;
  channels: Channel[];
  currentChannel: Channel | null;
  unreadChannels: Set<string>;
  mentionChannels: Set<string>;
  mutedChannels: Set<string>;
  workspaceMembers: WorkspaceMember[];
  currentUserId: string | undefined;
  user: { display_name: string; email: string } | null;
  dmConversations: DmConversation[];
  currentDmPartnerId: string | null;
  unreadDmPartners: Set<string>;
  onSelectChannel: (ch: Channel) => void;
  onCreateChannel: (name: string) => Promise<void>;
  onToggleMute: (channelId: string, muted: boolean) => void;
  onOpenDm: (userId: string) => void;
  onOpenMembers: () => void;
  onOpenSettings: () => void;
  onOpenProfile: () => void;
  onOpenNotifications: () => void;
  onLogout: () => void;
}

function usePresenceDot(userId: string) {
  const status = usePresenceStore((s) => s.getStatus(userId));
  return status === 'online' ? 'text-green-500' : status === 'away' ? 'text-amber-500' : 'text-slate-600';
}

function DmConversationButton({
  conv,
  isActive,
  isUnread,
  onSelect,
}: {
  conv: DmConversation;
  isActive: boolean;
  isUnread: boolean;
  onSelect: (partnerId: string) => void;
}) {
  const { getUser } = useUserCache();
  const partner = getUser(conv.partner_id);
  const dotColor = usePresenceDot(conv.partner_id);
  const name = displayNameOf(partner?.display_name);

  return (
    <button
      onClick={() => onSelect(conv.partner_id)}
      className={`w-full px-3 py-1.5 flex items-center gap-2 text-sm transition cursor-pointer ${
        isActive
          ? 'bg-purple-600/20 text-white'
          : isUnread
            ? 'text-white font-semibold hover:bg-slate-700/30'
            : 'text-slate-400 hover:bg-slate-700/30 hover:text-slate-200'
      }`}
    >
      <Circle className={`w-2.5 h-2.5 fill-current ${dotColor} shrink-0`} />
      <span className="truncate">{name}</span>
      {isUnread && !isActive && <span className="ml-auto w-2 h-2 bg-purple-400 rounded-full shrink-0" />}
    </button>
  );
}

function SidebarUser({ userId, onOpenDm }: { userId: string; onOpenDm: (id: string) => void }) {
  const { getUser } = useUserCache();
  const cached = getUser(userId);
  const dotColor = usePresenceDot(userId);

  const name = displayNameOf(cached?.display_name);

  return (
    <button
      onClick={() => onOpenDm(userId)}
      className="w-full px-3 py-1 flex items-center gap-2 text-sm text-slate-400 hover:bg-slate-700/30 hover:text-slate-200 transition cursor-pointer"
      title={`Message ${name}`}
    >
      <Circle className={`w-2.5 h-2.5 fill-current ${dotColor} shrink-0`} />
      <span className="truncate">{name}</span>
    </button>
  );
}

export default function ChannelSidebar({
  currentWorkspace,
  channels,
  currentChannel,
  unreadChannels,
  mentionChannels,
  mutedChannels,
  workspaceMembers,
  currentUserId,
  user,
  dmConversations,
  currentDmPartnerId,
  unreadDmPartners,
  onSelectChannel,
  onCreateChannel,
  onToggleMute,
  onOpenDm,
  onOpenMembers,
  onOpenSettings,
  onOpenProfile,
  onOpenNotifications,
  onLogout,
}: Props) {
  const { data: unreadNotifCount = 0 } = useUnreadNotificationCount(currentWorkspace?.id ?? null);
  const navigate = useNavigate();
  const { instances, activeInstanceUrl } = useInstanceStore();
  const currentInstance = instances.find((i) => i.url === activeInstanceUrl);
  const isInstanceAdmin = currentInstance?.user.is_instance_admin ?? false;

  const [wsDropdownOpen, setWsDropdownOpen] = useState(false);
  const [showNewChannel, setShowNewChannel] = useState(false);
  const [newChannelName, setNewChannelName] = useState('');
  const [showDmPicker, setShowDmPicker] = useState(false);
  const [dmSearch, setDmSearch] = useState('');

  const closeDmPicker = () => {
    setShowDmPicker(false);
    setDmSearch('');
  };

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
          {!muted && mentionChannels.has(ch.id) && (
            <span className="ml-auto w-5 h-5 bg-red-500 rounded-full shrink-0 flex items-center justify-center text-[10px] font-bold text-white">
              @
            </span>
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
        className="w-60 bg-slate-800/50 flex flex-col border-r border-slate-700/50"
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
          {dmConversations.length === 0 ? (
            <div className="px-3 py-1.5 text-xs text-slate-400">No conversations yet</div>
          ) : (
            dmConversations.map((conv) => (
              <DmConversationButton
                key={conv.partner_id}
                conv={conv}
                isActive={currentDmPartnerId === conv.partner_id}
                isUnread={unreadDmPartners.has(conv.partner_id)}
                onSelect={onOpenDm}
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
                  <SidebarUser key={m.user_id} userId={m.user_id} onOpenDm={onOpenDm} />
                ))}
            </>
          )}
        </div>

        <div className="px-3 py-3 border-t border-slate-700/50 flex items-center gap-2">
          <button
            onClick={onOpenProfile}
            className="w-8 h-8 rounded-full bg-purple-600 flex items-center justify-center text-sm font-bold shrink-0 hover:ring-2 hover:ring-purple-400 transition cursor-pointer"
            title="Edit profile"
          >
            {user?.display_name?.charAt(0).toUpperCase() || '?'}
          </button>
          <button
            onClick={onOpenProfile}
            className="flex-1 min-w-0 text-left hover:bg-slate-700/30 rounded px-1 -mx-1 transition cursor-pointer"
            title="Edit profile"
          >
            <div className="text-sm font-medium truncate">{user?.display_name}</div>
            <div className="text-xs text-slate-400 truncate">{user?.email}</div>
          </button>
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
          <h2 className="text-lg font-bold mb-4">New Message</h2>
          <input
            type="text"
            value={dmSearch}
            onChange={(e) => setDmSearch(e.target.value)}
            placeholder="Search people…"
            aria-label="Search people"
            className="w-full px-3 py-2 mb-3 bg-slate-700/50 border border-slate-600 rounded-lg text-sm text-white placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-purple-500"
          />
          <div className="flex flex-col gap-1 max-h-64 overflow-y-auto">
            {dmCandidates.length === 0 ? (
              <div className="px-3 py-4 text-sm text-slate-400 text-center">No people found</div>
            ) : (
              dmCandidates.map((m) => {
                const isSelf = m.user_id === currentUserId;
                return (
                  <button
                    key={m.user_id}
                    onClick={() => {
                      closeDmPicker();
                      onOpenDm(m.user_id);
                    }}
                    className="w-full px-3 py-2 flex items-center gap-3 rounded-lg hover:bg-slate-700/50 text-left transition"
                  >
                    <div className="w-8 h-8 rounded-full bg-slate-600 flex items-center justify-center text-sm font-bold shrink-0">
                      {(m.display_name || m.email).charAt(0).toUpperCase()}
                    </div>
                    <div className="min-w-0">
                      <div className="text-sm font-medium truncate">
                        {m.display_name || m.email}
                        {isSelf && <span className="text-slate-400 font-normal"> (you)</span>}
                      </div>
                      <div className="text-xs text-slate-400 truncate">{m.email}</div>
                    </div>
                  </button>
                );
              })
            )}
          </div>
          <div className="mt-4 flex justify-end">
            <button onClick={closeDmPicker} className="px-4 py-2 text-slate-400 hover:text-white transition">
              Cancel
            </button>
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
    </>
  );
}
