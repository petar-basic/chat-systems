import type { TFunction } from 'i18next';
import type { MentionItem } from './MentionDropdown';
import type { Channel, WorkspaceMember } from '@/stores/workspace';
import type { UserGroup } from '@/features/workspace';
import { BROADCAST_CONFIRM_ABOVE } from '@/shared/constants';
import { mentionsEveryone } from '@/lib/mentions';

function broadcastMentions(t: TFunction<'messages'>): MentionItem[] {
  return [
    { id: 'channel', label: 'channel', type: 'broadcast', hint: t('mentions.channel') },
    { id: 'here', label: 'here', type: 'broadcast', hint: t('mentions.here') },
    { id: 'everyone', label: 'everyone', type: 'broadcast', hint: t('mentions.channel') },
  ];
}

export function buildMentionItems(
  members: WorkspaceMember[],
  channels: Channel[],
  isDm: boolean,
  t: TFunction<'messages'>,
  groups: UserGroup[] = [],
  canBroadcast = true,
): MentionItem[] {
  return [
    ...(isDm || !canBroadcast ? [] : broadcastMentions(t)),
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
          hint: t('mentions.groupSize', { count: g.member_count }),
        }))),
    ...members.map((m) => ({
      id: m.user_id,
      label: m.display_name || m.email,
      type: 'user' as const,
      ...(m.title ? { title: m.title } : {}),
    })),
    ...channels.flatMap((c) => (c.name ? [{ id: c.id, label: c.name, type: 'channel' as const }] : [])),
  ];
}

export function matchesMention(item: MentionItem, query: string): boolean {
  const needle = query.toLowerCase();
  return item.label.toLowerCase().includes(needle) || !!item.title?.toLowerCase().includes(needle);
}

export function asksBeforeBroadcast(markdown: string, channelMemberCount: number): boolean {
  return channelMemberCount > BROADCAST_CONFIRM_ABOVE && mentionsEveryone(markdown);
}
