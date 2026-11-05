import { Hash, Lock } from 'lucide-react';
import type { ChannelActivity } from '@/features/workspace/hooks/useAnalytics';
import { dayLabel, lastMessageDay } from './days';
import { SortableTable, type Column } from './SortableTable';

export function ChannelLabel({ name, kind }: { name: string | null | undefined; kind: string }) {
  const Icon = kind === 'private' ? Lock : Hash;
  return (
    <span className="inline-flex items-center gap-1.5">
      <Icon className="h-3.5 w-3.5 text-muted" aria-label={kind === 'private' ? 'Private' : undefined} />
      {name ?? 'channel'}
    </span>
  );
}

const COLUMNS: Column<ChannelActivity>[] = [
  {
    key: 'name',
    label: 'Channel',
    value: (c) => c.name ?? '',
    render: (c) => (
      <span className="inline-flex items-center gap-2">
        <ChannelLabel name={c.name} kind={c.channel_type} />
        {c.is_archived && <span className="rounded bg-raised px-1.5 text-xs text-muted">archived</span>}
      </span>
    ),
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
