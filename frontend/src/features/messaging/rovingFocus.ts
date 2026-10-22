import { createContext, useContext, useSyncExternalStore } from 'react';

export interface RovingStore {
  stop: () => string | null;
  focus: (id: string) => void;
  setFallback: (id: string | null) => void;
  subscribe: (listener: () => void) => () => void;
}

export function createRovingStore(): RovingStore {
  let active: string | null = null;
  let fallback: string | null = null;
  const listeners = new Set<() => void>();
  const notify = () => listeners.forEach((listener) => listener());
  return {
    stop: () => active ?? fallback,
    focus: (id) => {
      if (active === id) return;
      active = id;
      notify();
    },
    setFallback: (id) => {
      if (fallback === id) return;
      fallback = id;
      if (!active) notify();
    },
    subscribe: (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
}

export const RovingContext = createContext<RovingStore | null>(null);

const alwaysStop = () => () => {};

export function useIsTabStop(id: string): boolean {
  const store = useContext(RovingContext);
  return useSyncExternalStore(store?.subscribe ?? alwaysStop, () => (store ? store.stop() === id : true));
}

export function useRovingFocus(): (id: string) => void {
  const store = useContext(RovingContext);
  return (id) => store?.focus(id);
}
