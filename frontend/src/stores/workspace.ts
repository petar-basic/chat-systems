import { create } from 'zustand';
import { instanceManager } from '../lib/instances';
import { wsClient } from '../lib/ws';
import { useInstanceStore } from './instances';

export interface Workspace {
  id: string;
  name: string;
  slug: string;
  description: string | null;
  icon_url: string | null;
  deleted_at?: string | null;
  instanceUrl: string;
}

export interface Channel {
  id: string;
  workspace_id: string;
  name: string;
  topic: string | null;
  channel_type: string;
  is_default: boolean;
  muted?: boolean;
}

export interface Reaction {
  id: string;
  message_id: string;
  user_id: string;
  emoji: string;
  created_at: string;
}

export interface MessageMetadata {
  kind?: string;
  huddle_id?: string;
  initiator_id?: string;
}

export interface Message {
  id: string;
  channel_id: string;
  user_id: string;
  content: string;
  created_at: string;
  updated_at: string;
  deleted_at: string | null;
  reactions?: Reaction[];
  thread_parent_id: string | null;
  reply_count: number;
  is_pinned: boolean;
  metadata?: MessageMetadata;
  pending?: boolean;
  failed?: boolean;
}

export type WorkspaceRole = 'owner' | 'admin' | 'channel_admin' | 'member' | 'guest';

export interface WorkspaceMember {
  workspace_id: string;
  user_id: string;
  role: string;
  joined_at: string;
  email: string;
  display_name: string | null;
  avatar_url: string | null;
}

interface WorkspaceState {
  currentWorkspace: Workspace | null;
  currentChannel: Channel | null;
  currentUserRole: WorkspaceRole | null;
  unreadChannels: Set<string>;
  mentionChannels: Set<string>;
  mutedChannels: Set<string>;
  currentDmPartnerId: string | null;
  unreadDmPartners: Set<string>;
  currentUserId: string | null;
  activeHuddleChannels: Map<string, { huddleId: string; initiatorId: string }>;

  selectWorkspace: (ws: Workspace) => Promise<void>;
  selectChannel: (ch: Channel) => void;
  selectDmPartner: (userId: string | null) => void;
  setCurrentUserRole: (role: WorkspaceRole | null) => void;
  setCurrentUserId: (id: string | null) => void;
  setChannelHuddle: (channelId: string, info: { huddleId: string; initiatorId: string }) => void;
  clearChannelHuddle: (channelId: string) => void;
  replaceActiveHuddleChannels: (
    entries: Array<{ channelId: string; huddleId: string; initiatorId: string }>,
  ) => void;
  markChannelRead: (channelId: string) => void;
  markDmRead: (partnerId: string) => void;
  markDmUnread: (partnerId: string) => void;
  hydrateUnreadDms: (partnerIds: string[]) => void;
  hydrateUnreadChannels: (channelIds: string[]) => void;
  hydrateMutedChannels: (channelIds: string[]) => void;
  setChannelMuted: (channelId: string, muted: boolean) => void;
}

function getWsClient(ws: Workspace | null) {
  if (ws?.instanceUrl) return instanceManager.get(ws.instanceUrl).ws;
  return wsClient;
}

export const useWorkspaceStore = create<WorkspaceState>((set, get) => ({
  currentWorkspace: null,
  currentChannel: null,
  currentUserRole: null,
  unreadChannels: new Set<string>(),
  mentionChannels: new Set<string>(),
  mutedChannels: new Set<string>(),
  currentDmPartnerId: null,
  unreadDmPartners: new Set<string>(),
  currentUserId: null,
  activeHuddleChannels: new Map<string, { huddleId: string; initiatorId: string }>(),

  selectWorkspace: async (ws) => {
    set({ currentWorkspace: ws, currentChannel: null, currentUserRole: null, currentDmPartnerId: null });
    getWsClient(ws).subscribe(ws.id);

    useInstanceStore.getState().setActiveInstance(ws.instanceUrl);
  },

  selectChannel: (ch) => {
    set({ currentChannel: ch, currentDmPartnerId: null });
    const ws = get().currentWorkspace;
    getWsClient(ws).joinChannel(ch.id);
  },

  selectDmPartner: (userId) => {
    set({ currentDmPartnerId: userId, currentChannel: null });
  },

  setCurrentUserRole: (role) => set({ currentUserRole: role }),

  setCurrentUserId: (id) => set({ currentUserId: id }),

  setChannelHuddle: (channelId, info) =>
    set((s) => {
      const next = new Map(s.activeHuddleChannels);
      next.set(channelId, info);
      return { activeHuddleChannels: next };
    }),

  clearChannelHuddle: (channelId) =>
    set((s) => {
      if (!s.activeHuddleChannels.has(channelId)) return s;
      const next = new Map(s.activeHuddleChannels);
      next.delete(channelId);
      return { activeHuddleChannels: next };
    }),

  replaceActiveHuddleChannels: (entries) =>
    set(() => {
      const next = new Map<string, { huddleId: string; initiatorId: string }>();
      for (const e of entries) {
        next.set(e.channelId, { huddleId: e.huddleId, initiatorId: e.initiatorId });
      }
      return { activeHuddleChannels: next };
    }),

  markChannelRead: (channelId) => {
    set((s) => {
      const nextUnread = new Set(s.unreadChannels);
      nextUnread.delete(channelId);
      const nextMention = new Set(s.mentionChannels);
      nextMention.delete(channelId);
      return { unreadChannels: nextUnread, mentionChannels: nextMention };
    });
  },

  markDmRead: (partnerId) => {
    set((s) => {
      const next = new Set(s.unreadDmPartners);
      next.delete(partnerId);
      return { unreadDmPartners: next };
    });
  },

  markDmUnread: (partnerId) => {
    set((s) => {
      const next = new Set(s.unreadDmPartners);
      next.add(partnerId);
      return { unreadDmPartners: next };
    });
  },

  hydrateUnreadDms: (partnerIds) => {
    set((s) => {
      const next = new Set(partnerIds);
      if (next.size === s.unreadDmPartners.size && [...next].every((id) => s.unreadDmPartners.has(id))) {
        return s;
      }
      return { unreadDmPartners: next };
    });
  },

  hydrateUnreadChannels: (channelIds) => {
    set((s) => {
      const next = new Set(s.unreadChannels);
      let changed = false;
      for (const id of channelIds) {
        if (!next.has(id)) {
          next.add(id);
          changed = true;
        }
      }
      return changed ? { unreadChannels: next } : s;
    });
  },

  hydrateMutedChannels: (channelIds) => {
    set((s) => {
      const next = new Set(channelIds);
      if (next.size === s.mutedChannels.size && [...next].every((id) => s.mutedChannels.has(id))) {
        return s;
      }
      return { mutedChannels: next };
    });
  },

  setChannelMuted: (channelId, muted) => {
    set((s) => {
      const next = new Set(s.mutedChannels);
      if (muted) next.add(channelId);
      else next.delete(channelId);
      return { mutedChannels: next };
    });
  },
}));
