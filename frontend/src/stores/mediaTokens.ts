import { create } from 'zustand';

interface MediaTokensState {
  tokens: Record<string, string>;
  put: (instanceUrl: string, token: string) => void;
  drop: (instanceUrl: string) => void;
}

export const useMediaTokenStore = create<MediaTokensState>((set) => ({
  tokens: {},
  put: (instanceUrl, token) => set((s) => ({ tokens: { ...s.tokens, [instanceUrl]: token } })),
  drop: (instanceUrl) =>
    set((s) => ({
      tokens: Object.fromEntries(Object.entries(s.tokens).filter(([url]) => url !== instanceUrl)),
    })),
}));
