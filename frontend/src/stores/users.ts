import { create } from 'zustand';

export interface UserInfo {
  id: string;
  email: string;
  display_name: string;
  avatar_url: string | null;
  bio?: string | null;
  title?: string | null;
  pronouns?: string | null;
  status_emoji?: string | null;
  status_text?: string | null;
  notifications_paused_until?: string | null;
  timezone?: string | null;
  is_bot?: boolean;
  left?: boolean;
  profile_fields?: Record<string, string>;
}

interface UserCacheState {
  users: Map<string, UserInfo>;
  populateUsers: (users: UserInfo[]) => void;
  getUser: (id: string) => UserInfo | undefined;
}

export const useUserCache = create<UserCacheState>((set, get) => ({
  users: new Map(),

  populateUsers: (users: UserInfo[]) => {
    set((s) => {
      const next = new Map(s.users);
      for (const u of users) {
        next.set(u.id, u);
      }
      return { users: next };
    });
  },

  getUser: (id: string) => get().users.get(id),
}));
