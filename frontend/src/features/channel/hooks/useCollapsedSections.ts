import { useState } from 'react';

export type SidebarSection = 'channels' | 'dms' | 'people';

const STORAGE_KEY = 'sidebar-collapsed';

function readStored(): Record<SidebarSection, boolean> {
  try {
    return {
      channels: false,
      dms: false,
      people: false,
      ...JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '{}'),
    };
  } catch {
    return { channels: false, dms: false, people: false };
  }
}

// A flat scroll is fine with six channels and unusable with two hundred, and
// which sections somebody keeps shut is a preference that should survive a
// reload.
export function useCollapsedSections() {
  const [collapsed, setCollapsed] = useState<Record<SidebarSection, boolean>>(readStored);
  const toggleSection = (section: SidebarSection) => {
    setCollapsed((current) => {
      const next = { ...current, [section]: !current[section] };
      try {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
      } catch {
        // A browser refusing storage is not a reason to refuse the click.
      }
      return next;
    });
  };
  return { collapsed, toggleSection };
}
