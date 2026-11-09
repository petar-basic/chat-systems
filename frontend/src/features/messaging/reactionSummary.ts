import type { TFunction } from 'i18next';

const NAMED_LIMIT = 3;

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
  t: TFunction<'messages'>,
): string[] {
  const names: string[] = [];
  let self = false;

  for (const id of userIds) {
    if (id === currentUserId) {
      self = true;
      continue;
    }
    names.push(nameOf(id) || t('reactions.someone'));
  }

  return self ? [t('reactions.you'), ...names] : names;
}

export function joinReactors(names: string[], t: TFunction<'messages'>): string {
  if (names.length === 0) return '';
  if (names.length === 1) return names[0];

  const shown = names.slice(0, NAMED_LIMIT);
  const rest = names.length - shown.length;

  const separator = t('reactions.separator');
  if (rest > 0) return t('reactions.others', { names: shown.join(separator), count: rest });

  return t('reactions.pair', { first: shown.slice(0, -1).join(separator), last: shown[shown.length - 1] });
}
