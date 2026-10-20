const NAMED_LIMIT = 3;
const UNKNOWN = 'Someone';

export interface ReactionGroup {
  emoji: string;
  count: number;
  hasOwn: boolean;
  userIds: string[];
}

export function reactorNames(
  userIds: string[],
  currentUserId: string,
  nameOf: (userId: string) => string | undefined,
): string[] {
  const names: string[] = [];
  let self = false;

  for (const id of userIds) {
    if (id === currentUserId) {
      self = true;
      continue;
    }
    names.push(nameOf(id) || UNKNOWN);
  }

  return self ? ['You', ...names] : names;
}

export function joinReactors(names: string[]): string {
  if (names.length === 0) return '';
  if (names.length === 1) return names[0];

  const shown = names.slice(0, NAMED_LIMIT);
  const rest = names.length - shown.length;

  if (rest > 0) return `${shown.join(', ')} and ${rest} ${rest === 1 ? 'other' : 'others'}`;

  return `${shown.slice(0, -1).join(', ')} and ${shown[shown.length - 1]}`;
}
