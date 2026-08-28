import { create } from 'zustand';
import { logger } from '@/lib/logger';

const STORAGE_KEY = 'chat_theme';

export type ThemeMode = 'system' | 'light' | 'dark';
export type ResolvedTheme = 'light' | 'dark';

const THEME_COLOR: Record<ResolvedTheme, string> = {
  dark: '#0f172a',
  light: '#eef1f6',
};

function systemQuery(): MediaQueryList | null {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return null;
  return window.matchMedia('(prefers-color-scheme: light)');
}

function load(): ThemeMode {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw === 'light' || raw === 'dark' || raw === 'system' ? raw : 'system';
  } catch {
    return 'system';
  }
}

function resolve(mode: ThemeMode): ResolvedTheme {
  if (mode !== 'system') return mode;
  return systemQuery()?.matches ? 'light' : 'dark';
}

function apply(resolved: ResolvedTheme) {
  if (typeof document === 'undefined') return;
  document.documentElement.dataset.theme = resolved;
  document.querySelector('meta[name="theme-color"]')?.setAttribute('content', THEME_COLOR[resolved]);
}

interface ThemeState {
  mode: ThemeMode;
  resolved: ResolvedTheme;
  setMode: (mode: ThemeMode) => void;
}

export const useThemeStore = create<ThemeState>((set) => {
  const mode = load();
  const resolved = resolve(mode);
  apply(resolved);

  systemQuery()?.addEventListener('change', () => {
    set((s) => {
      if (s.mode !== 'system') return s;
      const next = resolve('system');
      apply(next);
      return { resolved: next };
    });
  });

  return {
    mode,
    resolved,
    setMode: (next) => {
      try {
        localStorage.setItem(STORAGE_KEY, next);
      } catch (err) {
        logger.error('useThemeStore', 'setMode', err);
      }
      const nextResolved = resolve(next);
      apply(nextResolved);
      set({ mode: next, resolved: nextResolved });
    },
  };
});
