import type { MentionItem } from './MentionDropdown';
import type { Channel, WorkspaceMember } from '@/stores/workspace';
import type { UserGroup } from '@/hooks/queries/useUserGroups';

export const BROADCAST_MENTIONS: MentionItem[] = [
  { id: 'channel', label: 'channel', type: 'broadcast', hint: 'everyone in here' },
  { id: 'here', label: 'here', type: 'broadcast', hint: 'those online now' },
  { id: 'everyone', label: 'everyone', type: 'broadcast', hint: 'everyone in here' },
];

export function buildMentionItems(
  members: WorkspaceMember[],
  channels: Channel[],
  isDm: boolean,
  groups: UserGroup[] = [],
): MentionItem[] {
  return [
    ...(isDm ? [] : BROADCAST_MENTIONS),
    // A group is only meaningful where it can fan out, and a DM has no channel
    // membership to intersect with.
    ...(isDm
      ? []
      : groups.map((g) => ({
          // The `group:` prefix is what keeps a group id from being read as a
          // user id on the way back in.
          id: `group:${g.id}`,
          label: g.handle,
          type: 'group' as const,
          hint: `${g.member_count} ${g.member_count === 1 ? 'person' : 'people'}`,
        }))),
    ...members.map((m) => ({
      id: m.user_id,
      label: m.display_name || m.email,
      type: 'user' as const,
    })),
    ...channels.flatMap((c) => (c.name ? [{ id: c.id, label: c.name, type: 'channel' as const }] : [])),
  ];
}
