import { create } from 'zustand';
import { globalEventBus } from '../lib/globalEventBus';

export type PresenceStatus = 'online' | 'away' | 'offline';

const OFFLINE_GRACE_MS = 5000;

interface PresenceState {
  statuses: Record<string, PresenceStatus>;
  setStatus: (userId: string, status: PresenceStatus) => void;
  getStatus: (userId: string) => PresenceStatus;
  initPresenceListener: () => () => void;
}

const offlineTimers: Map<string, ReturnType<typeof setTimeout>> = new Map();

export const usePresenceStore = create<PresenceState>((set, get) => ({
  statuses: {},

  setStatus: (userId, status) => {
    set((s) => ({
      statuses: { ...s.statuses, [userId]: status },
    }));
  },

  getStatus: (userId) => {
    return get().statuses[userId] || 'offline';
  },

  initPresenceListener: () => {
    const unsub = globalEventBus.on('presence.changed', (event) => {
      const userId = event.user_id as string;
      const status = event.status as PresenceStatus;
      if (!userId || !status) return;

      if (status === 'offline') {
        if (!offlineTimers.has(userId)) {
          const timer = setTimeout(() => {
            offlineTimers.delete(userId);
            set((s) => ({ statuses: { ...s.statuses, [userId]: 'offline' } }));
          }, OFFLINE_GRACE_MS);
          offlineTimers.set(userId, timer);
        }
      } else {
        const pending = offlineTimers.get(userId);
        if (pending) {
          clearTimeout(pending);
          offlineTimers.delete(userId);
        }
        set((s) => ({ statuses: { ...s.statuses, [userId]: status } }));
      }
    });

    const unsubBatch = globalEventBus.on('presence.batch', (event) => {
      const users = event.users as Array<{ user_id: string; status: PresenceStatus }>;
      if (Array.isArray(users)) {
        set((s) => {
          const next = { ...s.statuses };
          for (const u of users) {
            next[u.user_id] = u.status;
          }
          return { statuses: next };
        });
      }
    });

    return () => {
      unsub();
      unsubBatch();
      offlineTimers.forEach((t) => clearTimeout(t));
      offlineTimers.clear();
    };
  },
}));
