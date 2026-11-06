import { Archive, Hash, Lock } from 'lucide-react';
import type { ChannelActivity } from '@/features/workspace/hooks/useAnalytics';
import { dayLabel, dayOf } from './days';
import { SortableTable, type Column } from './SortableTable';

function ChannelLabel({ channel }: { channel: ChannelActivity }) {
  const Icon = channel.is_archived ? Archive : channel.channel_type === 'private' ? Lock : Hash;
  const kind = [channel.is_archived && 'Archived', channel.channel_type === 'private' && 'Private']
    .filter(Boolean)
    .join(', ');
  return (
    <span className={`inline-flex items-center gap-1.5 ${channel.is_archived ? 'text-muted' : ''}`}>
      <Icon className="h-3.5 w-3.5 shrink-0 text-muted" aria-label={kind || undefined} />
      <span className="max-w-32 truncate" data-tooltip={channel.name ?? undefined}>
        {channel.name ?? 'channel'}
      </span>
    </span>
  );
}

function columns(guests: boolean): Column<ChannelActivity>[] {
  return [
    {
      key: 'name',
      label: 'Channel',
      value: (c) => c.name ?? '',
      render: (c) => <ChannelLabel channel={c} />,
    },
    {
      key: 'created',
      label: 'Created',
      value: (c) => dayOf(c.created_at),
      render: (c) => <span className="text-muted">{dayLabel(dayOf(c.created_at))}</span>,
    },
    { key: 'members', label: 'Members', numeric: true, value: (c) => c.members },
    ...(guests
      ? [{ key: 'guests', label: 'Guests', numeric: true, value: (c: ChannelActivity) => c.guests }]
      : []),
    { key: 'messages', label: 'Messages', numeric: true, value: (c) => c.messages },
    { key: 'posters', label: 'Posted', numeric: true, value: (c) => c.posters },
    { key: 'viewers', label: 'Viewed', numeric: true, value: (c) => c.viewers },
    { key: 'reactions', label: 'Reactions', numeric: true, value: (c) => c.reactions },
    {
      key: 'lastMessage',
      label: 'Last message',
      value: (c) => dayOf(c.last_message_at),
      render: (c) => (
        <span className="text-muted">{c.last_message_at ? dayLabel(dayOf(c.last_message_at)) : 'Never'}</span>
      ),
    },
  ];
}

export function ChannelTable({ rows, guests }: { rows: ChannelActivity[]; guests: boolean }) {
  return (
    <SortableTable
      rows={rows}
      columns={columns(guests)}
      rowKey={(c) => c.channel_id}
      initialSort="messages"
      qa="analytics-channels"
      rowQa="analytics-channel-row"
    />
  );
}
