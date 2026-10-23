import type { WorkspaceMember } from '@/stores/workspace';
import { displayNameOf } from '@/lib/userHelpers';

export interface Person {
  id: string;
  name: string;
  avatarUrl?: string | null;
}

export interface People {
  get: (userId: string) => Person;
  nameOf: (userId: string) => string;
  eligible: Person[];
}

export function peopleFrom(members: WorkspaceMember[], eligibleIds: string[] | null): People {
  const byId = new Map<string, Person>(
    members.map((m) => [
      m.user_id,
      { id: m.user_id, name: displayNameOf(m.display_name), avatarUrl: m.avatar_url },
    ]),
  );
  const get = (userId: string): Person => byId.get(userId) ?? { id: userId, name: displayNameOf(null) };
  const allowed = eligibleIds ? new Set(eligibleIds) : null;
  const eligible = [...byId.values()]
    .filter((person) => !allowed || allowed.has(person.id))
    .sort((a, b) => a.name.localeCompare(b.name));
  return { get, nameOf: (userId) => get(userId).name, eligible };
}
