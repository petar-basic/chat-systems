import { create } from 'zustand';

export const ANSWER_WAIT_MS = 30 * 60 * 1000;

export interface CommandAnswer {
  command: string;
  text: string;
  pending: boolean;
  since: number;
}

interface CommandAnswerState {
  answers: Record<string, CommandAnswer>;
  show: (place: string, answer: Omit<CommandAnswer, 'since'>) => void;
  settle: (place: string) => void;
  dismiss: (place: string) => void;
  giveUp: (place?: string) => void;
}

export const answerPlace = (channelId: string, threadParentId?: string | null) =>
  threadParentId ? `thread:${threadParentId}` : channelId;

const without = (answers: Record<string, CommandAnswer>, place: string) =>
  Object.fromEntries(Object.entries(answers).filter(([key]) => key !== place));

export const useCommandAnswerStore = create<CommandAnswerState>((set) => ({
  answers: {},
  show: (place, answer) =>
    set((s) => ({ answers: { ...s.answers, [place]: { ...answer, since: Date.now() } } })),
  settle: (place) => set((s) => (s.answers[place]?.pending ? { answers: without(s.answers, place) } : s)),
  dismiss: (place) => set((s) => (s.answers[place] ? { answers: without(s.answers, place) } : s)),
  giveUp: (place) =>
    set((s) => {
      const waiting = Object.entries(s.answers).filter(
        ([key, answer]) => answer.pending && (place === undefined || key === place),
      );
      if (waiting.length === 0) return s;
      const answers = { ...s.answers };
      for (const [key, answer] of waiting) {
        answers[key] = {
          command: answer.command,
          text: `No answer from /${answer.command}`,
          pending: false,
          since: Date.now(),
        };
      }
      return { answers };
    }),
}));
