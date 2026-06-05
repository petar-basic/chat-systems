import { create } from 'zustand';

interface UserInfo {
  id: string;
  email: string;
  display_name: string;
  avatar_url: string | null;
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
