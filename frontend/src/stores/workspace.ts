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
  description: string | null;
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

export interface BotIdentity {
  hook_id: string;
  name: string;
  icon_url?: string | null;
}

export interface MessageMetadata {
  kind?: string;
  huddle_id?: string;
  initiator_id?: string;
  bot?: BotIdentity;
}

export interface Message {
  id: string;
  channel_id: string;
  user_id: string;
  client_message_id?: string | null;
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
  unreadChannels: Set<string>;
  mentionChannels: Set<string>;
  /** How many, not just whether — the server now keeps an exact counter. */
  unreadCounts: Record<string, number>;
  mentionCounts: Record<string, number>;
  mutedChannels: Set<string>;
  currentConversationId: string | null;
  unreadConversations: Set<string>;
  currentUserId: string | null;
  activeHuddleChannels: Map<string, { huddleId: string; initiatorId: string }>;

  selectWorkspace: (ws: Workspace) => Promise<void>;
  selectChannel: (ch: Channel) => void;
  selectConversation: (conversationId: string | null) => void;
  setCurrentUserId: (id: string | null) => void;
  setChannelHuddle: (channelId: string, info: { huddleId: string; initiatorId: string }) => void;
  clearChannelHuddle: (channelId: string) => void;
  replaceActiveHuddleChannels: (
    entries: Array<{ channelId: string; huddleId: string; initiatorId: string }>,
  ) => void;
  markChannelRead: (channelId: string) => void;
  markConversationRead: (conversationId: string) => void;
  markConversationUnread: (conversationId: string) => void;
  hydrateUnreadConversations: (conversationIds: string[]) => void;
  hydrateUnreadChannels: (channelIds: string[]) => void;
  bumpChannelUnread: (channelId: string, isMention: boolean) => void;
  hydrateUnreadCounts: (
    counts: Array<{ channel_id: string; unread_count: number; mention_count: number }>,
  ) => void;
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
  unreadChannels: new Set<string>(),
  mentionChannels: new Set<string>(),
  unreadCounts: {},
  mentionCounts: {},
  mutedChannels: new Set<string>(),
  currentConversationId: null,
  unreadConversations: new Set<string>(),
  currentUserId: null,
  activeHuddleChannels: new Map<string, { huddleId: string; initiatorId: string }>(),

  selectWorkspace: async (ws) => {
    set({ currentWorkspace: ws, currentChannel: null, currentConversationId: null });
    getWsClient(ws).subscribe(ws.id);

    useInstanceStore.getState().setActiveInstance(ws.instanceUrl);
  },

  selectChannel: (ch) => {
    set({ currentChannel: ch, currentConversationId: null });
    const ws = get().currentWorkspace;
    getWsClient(ws).joinChannel(ch.id);
  },

  selectConversation: (conversationId) => {
    set({ currentConversationId: conversationId, currentChannel: null });
  },

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
      const unreadCounts = { ...s.unreadCounts };
      const mentionCounts = { ...s.mentionCounts };
      delete unreadCounts[channelId];
      delete mentionCounts[channelId];
      return { unreadChannels: nextUnread, mentionChannels: nextMention, unreadCounts, mentionCounts };
    });
  },

  bumpChannelUnread: (channelId, isMention) => {
    set((s) => ({
      unreadCounts: { ...s.unreadCounts, [channelId]: (s.unreadCounts[channelId] ?? 0) + 1 },
      mentionCounts: isMention
        ? { ...s.mentionCounts, [channelId]: (s.mentionCounts[channelId] ?? 0) + 1 }
        : s.mentionCounts,
    }));
  },

  hydrateUnreadCounts: (counts) => {
    set(() => {
      const unreadCounts: Record<string, number> = {};
      const mentionCounts: Record<string, number> = {};
      for (const row of counts) {
        if (row.unread_count > 0) unreadCounts[row.channel_id] = row.unread_count;
        if (row.mention_count > 0) mentionCounts[row.channel_id] = row.mention_count;
      }
      return { unreadCounts, mentionCounts };
    });
  },

  markConversationRead: (conversationId) => {
    set((s) => {
      const next = new Set(s.unreadConversations);
      next.delete(conversationId);
      return { unreadConversations: next };
    });
  },

  markConversationUnread: (conversationId) => {
    set((s) => {
      const next = new Set(s.unreadConversations);
      next.add(conversationId);
      return { unreadConversations: next };
    });
  },

  hydrateUnreadConversations: (conversationIds) => {
    set((s) => {
      const next = new Set(conversationIds);
      if (
        next.size === s.unreadConversations.size &&
        [...next].every((id) => s.unreadConversations.has(id))
      ) {
        return s;
      }
      return { unreadConversations: next };
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
