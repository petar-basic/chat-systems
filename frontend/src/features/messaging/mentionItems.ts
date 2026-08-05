import type { MentionItem } from './MentionDropdown';
import type { Channel, WorkspaceMember } from '@/stores/workspace';

export const BROADCAST_MENTIONS: MentionItem[] = [
  { id: 'channel', label: 'channel', type: 'broadcast', hint: 'everyone in here' },
  { id: 'here', label: 'here', type: 'broadcast', hint: 'those online now' },
  { id: 'everyone', label: 'everyone', type: 'broadcast', hint: 'everyone in here' },
];

export function buildMentionItems(
  members: WorkspaceMember[],
  channels: Channel[],
  isDm: boolean,
): MentionItem[] {
  return [
    ...(isDm ? [] : BROADCAST_MENTIONS),
    ...members.map((m) => ({
      id: m.user_id,
      label: m.display_name || m.email,
      type: 'user' as const,
    })),
    ...channels.map((c) => ({ id: c.id, label: c.name, type: 'channel' as const })),
  ];
}
