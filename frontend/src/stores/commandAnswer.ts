import { create } from 'zustand';

export interface CommandAnswer {
  channelId: string;
  text: string;
  pending: boolean;
}

interface CommandAnswerState {
  answer: CommandAnswer | null;
  show: (answer: CommandAnswer) => void;
  settle: (channelId: string) => void;
  dismiss: () => void;
}

export const useCommandAnswerStore = create<CommandAnswerState>((set) => ({
  answer: null,
  show: (answer) => set({ answer }),
  settle: (channelId) =>
    set((s) => (s.answer?.pending && s.answer.channelId === channelId ? { answer: null } : s)),
  dismiss: () => set({ answer: null }),
}));
