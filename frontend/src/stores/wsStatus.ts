import { create } from 'zustand';
import type { WsConnectionStatus } from '../lib/ws';

interface WsStatusState {
  statuses: Record<string, WsConnectionStatus>;
  setStatus: (instanceUrl: string, status: WsConnectionStatus) => void;
}

export const useWsStatusStore = create<WsStatusState>((set) => ({
  statuses: {},
  setStatus: (instanceUrl, status) => set((s) => ({ statuses: { ...s.statuses, [instanceUrl]: status } })),
}));
