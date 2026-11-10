import { QueryState } from '@/shared/components/QueryState/QueryState';
import { useTranslation } from 'react-i18next';
import type { Channel } from '@/stores/workspace';
import { dayLabel, dayOf } from '@/features/workspace';
import { useChannelAnalytics } from '../hooks/useChannelAnalytics';

function Stat({ label, value, hint, qa }: { label: string; value: string; hint?: string; qa: string }) {
  return (
    <div className="rounded-xl border border-line bg-surface px-4 py-3" data-qa={qa}>
      <div className="text-xs text-muted">{label}</div>
      <div className="text-xl font-semibold tabular-nums text-fg">{value}</div>
      {hint && <div className="text-xs text-muted">{hint}</div>}
    </div>
  );
}

export function ActivitySection({ channel }: { channel: Channel }) {
  const { t } = useTranslation('channel');
  const analytics = useChannelAnalytics(channel.id);
  const report = analytics.data;
  const activity = report?.channel;
  const of = activity
    ? t('details.activity.ofMembers', { members: activity.members.toLocaleString() })
    : undefined;
  const last = dayOf(activity?.last_message_at);

  return (
    <QueryState
      isLoading={analytics.isLoading}
      isError={analytics.isError}
      onRetry={() => void analytics.refetch()}
    >
      {report && activity && (
        <div className="space-y-4" data-qa="channel-activity">
          <p className="text-sm text-fg-soft">
            {t('details.activity.range', { from: dayLabel(report.from), to: dayLabel(report.to) })}
          </p>
          <div className="grid grid-cols-2 gap-3">
            <Stat
              label={t('details.activity.messages')}
              value={activity.messages.toLocaleString()}
              qa="channel-activity-messages"
            />
            <Stat
              label={t('details.activity.posters')}
              value={activity.posters.toLocaleString()}
              hint={of}
              qa="channel-activity-posters"
            />
            <Stat
              label={t('details.activity.viewers')}
              value={activity.viewers.toLocaleString()}
              hint={of}
              qa="channel-activity-viewers"
            />
            <Stat
              label={t('details.activity.last')}
              value={last ? dayLabel(last) : t('details.activity.noneYet')}
              qa="channel-activity-last"
            />
          </div>
        </div>
      )}
    </QueryState>
  );
}
