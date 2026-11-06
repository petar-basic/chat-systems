import { ROUTES } from '@/shared/constants';

const STORAGE_KEY = 'last-place';

interface Stored {
  workspaceId?: string;
  places: Record<string, string>;
}

function read(): Stored {
  try {
    const parsed = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '{}') as Partial<Stored>;
    return { workspaceId: parsed.workspaceId, places: parsed.places ?? {} };
  } catch {
    return { places: {} };
  }
}

export function rememberPlace(workspaceId: string, path: string) {
  const stored = read();
  if (stored.workspaceId === workspaceId && stored.places[workspaceId] === path) return;
  try {
    localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({ workspaceId, places: { ...stored.places, [workspaceId]: path } }),
    );
  } catch {
    return;
  }
}

export function lastWorkspaceId(): string | undefined {
  return read().workspaceId;
}

export function placeIn(workspaceId: string): string {
  const path = read().places[workspaceId];
  return path?.startsWith(`${ROUTES.workspace(workspaceId)}/`) ? path : ROUTES.workspace(workspaceId);
}
