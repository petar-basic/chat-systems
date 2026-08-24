import { create } from 'zustand';

interface UserGroupState {
  /// Ids in the `group:<uuid>` form that mentions carry, so the highlighter can
  /// compare without re-deriving the prefix on every message.
  selfGroupIds: Set<string>;
  populate: (ids: string[]) => void;
}

/**
 * A store rather than a query inside the renderer, for the same reason custom
 * emoji is: `MessageContent` renders once per message and stays free of server
 * state.
 */
export const useUserGroupStore = create<UserGroupState>((set) => ({
  selfGroupIds: new Set(),
  populate: (ids) => set({ selfGroupIds: new Set(ids) }),
}));
