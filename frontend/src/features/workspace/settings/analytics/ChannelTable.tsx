import { Archive, Hash, Lock } from 'lucide-react';
import type { ChannelActivity } from '@/features/workspace/hooks/useAnalytics';
import { dayLabel, lastMessageDay } from './days';
import { SortableTable, type Column } from './SortableTable';

function ChannelLabel({ channel }: { channel: ChannelActivity }) {
  const Icon = channel.is_archived ? Archive : channel.channel_type === 'private' ? Lock : Hash;
  const kind = [channel.is_archived && 'Archived', channel.channel_type === 'private' && 'Private']
    .filter(Boolean)
    .join(', ');
  return (
    <span className={`inline-flex items-center gap-1.5 ${channel.is_archived ? 'text-muted' : ''}`}>
      <Icon className="h-3.5 w-3.5 shrink-0 text-muted" aria-label={kind || undefined} />
      <span className="max-w-32 truncate" title={channel.name ?? undefined}>
        {channel.name ?? 'channel'}
      </span>
    </span>
  );
}

const COLUMNS: Column<ChannelActivity>[] = [
  {
    key: 'name',
    label: 'Channel',
    value: (c) => c.name ?? '',
    render: (c) => <ChannelLabel channel={c} />,
  },
  { key: 'members', label: 'Members', numeric: true, value: (c) => c.members },
  { key: 'messages', label: 'Messages', numeric: true, value: (c) => c.messages },
  { key: 'posters', label: 'Posted', numeric: true, value: (c) => c.posters },
  { key: 'readers', label: 'Read', numeric: true, value: (c) => c.readers },
  {
    key: 'lastMessage',
    label: 'Last message',
    value: (c) => lastMessageDay(c.last_message_at),
    render: (c) => (
      <span className="text-muted">
        {c.last_message_at ? dayLabel(lastMessageDay(c.last_message_at)) : 'Never'}
      </span>
    ),
  },
];

export function ChannelTable({ rows }: { rows: ChannelActivity[] }) {
  return (
    <SortableTable
      rows={rows}
      columns={COLUMNS}
      rowKey={(c) => c.channel_id}
      initialSort="messages"
      qa="analytics-channels"
      rowQa="analytics-channel-row"
    />
  );
}
