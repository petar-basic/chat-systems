import { create } from 'zustand';

interface ProfilePanelState {
  userId: string | null;
  opened: number;
  show: (userId: string) => void;
}

export const useProfilePanel = create<ProfilePanelState>((set) => ({
  userId: null,
  opened: 0,
  show: (userId) => set((s) => ({ userId, opened: s.opened + 1 })),
}));
