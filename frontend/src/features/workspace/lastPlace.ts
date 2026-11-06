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

export function rememberedTarget(
  workspaceId: string,
): { conversationId: string } | { channelId: string } | null {
  const rest = placeIn(workspaceId)
    .slice(ROUTES.workspace(workspaceId).length + 1)
    .split('/');
  if (rest[0] === 'c' && rest[1]) return { conversationId: rest[1] };
  if (rest.length === 1 && rest[0] && !['unreads', 'threads'].includes(rest[0]))
    return { channelId: rest[0] };
  return null;
}
