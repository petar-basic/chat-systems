import type { FormerMember, WorkspaceMember } from '../stores/workspace';
import type { UserInfo } from '@/stores/users';
import i18n from '@/lib/i18n';

export function displayNameOf(name: string | null | undefined): string {
  return name && name.trim() ? name : i18n.t('common:unknownUser');
}

export const AVATAR_COLORS = [
  'bg-purple-600',
  'bg-blue-600',
  'bg-green-600',
  'bg-amber-600',
  'bg-pink-600',
  'bg-teal-600',
  'bg-indigo-600',
  'bg-rose-600',
];

export function avatarColorFor(userId: string): string {
  const index = userId.split('').reduce((acc, ch) => acc + ch.charCodeAt(0), 0) % AVATAR_COLORS.length;
  return AVATAR_COLORS[index];
}

export function initialOf(name: string | null | undefined): string {
  const trimmed = name?.trim();
  return trimmed ? trimmed.charAt(0).toUpperCase() : '?';
}

export function matchesPerson(
  person: { display_name?: string | null; email?: string; title?: string | null },
  query: string,
): boolean {
  const needle = query.trim().toLowerCase();
  if (!needle) return true;
  return [person.display_name || person.email, person.title].some((field) =>
    field?.toLowerCase().includes(needle),
  );
}

export function getUserDisplay(userId: string, members: WorkspaceMember[]) {
  const member = members.find((m) => m.user_id === userId);
  const displayName = member?.display_name || i18n.t('common:unknownUser');
  const email = member?.email || '';

  return {
    member,
    displayName,
    email,
  };
}

export function knownPeople(members: WorkspaceMember[], former: FormerMember[]): UserInfo[] {
  const here = new Set(members.map((m) => m.user_id));
  return [
    ...former
      .filter((m) => !here.has(m.user_id))
      .map((m) => ({
        id: m.user_id,
        email: '',
        display_name: m.display_name ?? '',
        avatar_url: m.avatar_url ?? null,
        left: true,
      })),
    ...members.map((m) => ({
      id: m.user_id,
      email: m.email,
      display_name: m.display_name ?? '',
      avatar_url: m.avatar_url ?? null,
      bio: m.bio,
      title: m.title,
      pronouns: m.pronouns,
      status_emoji: m.status_emoji,
      status_text: m.status_text,
      notifications_paused_until: m.notifications_paused_until,
      timezone: m.timezone,
      is_bot: m.is_bot,
    })),
  ];
}
