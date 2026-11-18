import { create } from 'zustand';
import type { components } from '@/api/schema';

export type AppView = components['schemas']['AppView'];
export type ViewValues = Record<string, Record<string, string[]>>;

interface AppViewsState {
  stacks: Record<string, AppView[]>;
  values: Record<string, ViewValues>;
  replace: (workspaceId: string, views: AppView[]) => void;
  drop: (workspaceId: string, viewIds: string[]) => void;
  setValue: (viewId: string, blockId: string, actionId: string, value: string[]) => void;
}

function kept(values: Record<string, ViewValues>, stacks: Record<string, AppView[]>) {
  const live = new Set(Object.values(stacks).flatMap((stack) => stack.map((view) => view.id)));
  return Object.fromEntries(Object.entries(values).filter(([id]) => live.has(id)));
}

export const useAppViewsStore = create<AppViewsState>((set) => ({
  stacks: {},
  values: {},
  replace: (workspaceId, views) =>
    set((s) => {
      const stacks = { ...s.stacks, [workspaceId]: views };
      return { stacks, values: kept(s.values, stacks) };
    }),
  drop: (workspaceId, viewIds) =>
    set((s) => {
      const stack = s.stacks[workspaceId];
      if (!stack) return s;
      const stacks = { ...s.stacks, [workspaceId]: stack.filter((view) => !viewIds.includes(view.id)) };
      return { stacks, values: kept(s.values, stacks) };
    }),
  setValue: (viewId, blockId, actionId, value) =>
    set((s) => {
      const view = s.values[viewId] ?? {};
      return {
        values: {
          ...s.values,
          [viewId]: { ...view, [blockId]: { ...view[blockId], [actionId]: value } },
        },
      };
    }),
}));
