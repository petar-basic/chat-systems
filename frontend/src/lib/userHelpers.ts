import type { WorkspaceMember } from '../stores/workspace';
import { UNKNOWN_USER } from '@/shared/constants';

export function displayNameOf(name: string | null | undefined): string {
  return name && name.trim() ? name : UNKNOWN_USER;
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
  const displayName = member?.display_name || UNKNOWN_USER;
  const email = member?.email || '';

  return {
    member,
    displayName,
    email,
  };
}
