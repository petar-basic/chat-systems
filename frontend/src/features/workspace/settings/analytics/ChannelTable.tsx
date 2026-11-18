import { Archive, Hash, Lock } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';
import i18n from '@/lib/i18n';
import type { ChannelActivity } from '@/features/workspace/hooks/useAnalytics';
import { dayLabel, dayOf } from './days';
import { SortableTable, type Column } from './SortableTable';

function ChannelLabel({ channel }: { channel: ChannelActivity }) {
  const { t } = useTranslation('workspace');
  const Icon = channel.is_archived ? Archive : channel.channel_type === 'private' ? Lock : Hash;
  const kind = [
    channel.is_archived && t('settings.analytics.channels.archived'),
    channel.channel_type === 'private' && t('settings.analytics.channels.private'),
  ]
    .filter(Boolean)
    .join(i18n.t('common:listSeparator'));
  return (
    <span className={`inline-flex items-center gap-1.5 ${channel.is_archived ? 'text-muted' : ''}`}>
      <Icon className="h-3.5 w-3.5 shrink-0 text-muted" aria-label={kind || undefined} />
      <span className="max-w-32 truncate" data-tooltip={channel.name ?? undefined}>
        {channel.name ?? t('settings.analytics.channels.unnamed')}
      </span>
    </span>
  );
}

function columns(guests: boolean, t: TFunction<'workspace'>): Column<ChannelActivity>[] {
  return [
    {
      key: 'name',
      label: t('settings.analytics.channels.channel'),
      value: (c) => c.name ?? '',
      render: (c) => <ChannelLabel channel={c} />,
    },
    {
      key: 'created',
      label: t('settings.analytics.channels.created'),
      value: (c) => dayOf(c.created_at),
      render: (c) => <span className="text-muted">{dayLabel(dayOf(c.created_at))}</span>,
    },
    {
      key: 'members',
      label: t('settings.analytics.channels.members'),
      numeric: true,
      value: (c) => c.members,
    },
    ...(guests
      ? [
          {
            key: 'guests',
            label: t('settings.analytics.channels.guests'),
            numeric: true,
            value: (c: ChannelActivity) => c.guests,
          },
        ]
      : []),
    {
      key: 'messages',
      label: t('settings.analytics.channels.messages'),
      numeric: true,
      value: (c) => c.messages,
    },
    {
      key: 'posters',
      label: t('settings.analytics.channels.posted'),
      numeric: true,
      value: (c) => c.posters,
    },
    {
      key: 'viewers',
      label: t('settings.analytics.channels.viewed'),
      numeric: true,
      value: (c) => c.viewers,
    },
    {
      key: 'reactions',
      label: t('settings.analytics.channels.reactions'),
      numeric: true,
      value: (c) => c.reactions,
    },
    {
      key: 'lastMessage',
      label: t('settings.analytics.channels.lastMessage'),
      value: (c) => dayOf(c.last_message_at),
      render: (c) => (
        <span className="text-muted">
          {c.last_message_at ? dayLabel(dayOf(c.last_message_at)) : t('settings.analytics.channels.never')}
        </span>
      ),
    },
  ];
}

export function ChannelTable({ rows, guests }: { rows: ChannelActivity[]; guests: boolean }) {
  const { t } = useTranslation('workspace');
  return (
    <SortableTable
      rows={rows}
      columns={columns(guests, t)}
      rowKey={(c) => c.channel_id}
      initialSort="messages"
      qa="analytics-channels"
      rowQa="analytics-channel-row"
    />
  );
}
