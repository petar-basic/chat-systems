import { create } from 'zustand';

export type ServerProblem = { kind: 'needsNewerApp'; minVersion: string } | { kind: 'serverTooOld' };

interface ServerSupportState {
  problems: Record<string, ServerProblem>;
  report: (instanceUrl: string, problem: ServerProblem) => void;
}

export const useServerSupportStore = create<ServerSupportState>((set) => ({
  problems: {},
  report: (instanceUrl, problem) => set((s) => ({ problems: { ...s.problems, [instanceUrl]: problem } })),
}));
