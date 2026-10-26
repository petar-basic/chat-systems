import { useState } from 'react';
import type { SidebarLayout, BuiltinSection } from '@/features/channel/hooks/useSidebarLayout';

export type SidebarSection = BuiltinSection;

const STORAGE_KEY = 'sidebar-collapsed';
const NONE: Record<SidebarSection, boolean> = { channels: false, dms: false, people: false };

function readStored(): Record<SidebarSection, boolean> {
  try {
    return { ...NONE, ...JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '{}') };
  } catch {
    return NONE;
  }
}

function store(collapsed: Record<SidebarSection, boolean>) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(collapsed));
  } catch {
    return;
  }
}

export function useCollapsedSections(
  layout: SidebarLayout | undefined,
  save: (section: SidebarSection, collapsed: boolean) => void,
) {
  const [firstPaint] = useState(readStored);
  const collapsed: Record<SidebarSection, boolean> = layout
    ? {
        channels: layout.collapsed_builtins.includes('channels'),
        dms: layout.collapsed_builtins.includes('dms'),
        people: layout.collapsed_builtins.includes('people'),
      }
    : firstPaint;

  const toggleSection = (section: SidebarSection) => {
    const next = { ...collapsed, [section]: !collapsed[section] };
    store(next);
    save(section, next[section]);
  };

  return { collapsed, toggleSection };
}
