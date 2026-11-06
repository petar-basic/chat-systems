import { QueryState } from '@/shared/components/QueryState/QueryState';
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
  const analytics = useChannelAnalytics(channel.id);
  const report = analytics.data;
  const activity = report?.channel;
  const of = activity ? `of ${activity.members.toLocaleString()} members` : undefined;
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
            {dayLabel(report.from)} – {dayLabel(report.to)}. Counts only, never what anyone wrote.
          </p>
          <div className="grid grid-cols-2 gap-3">
            <Stat
              label="Messages"
              value={activity.messages.toLocaleString()}
              qa="channel-activity-messages"
            />
            <Stat
              label="Members who posted"
              value={activity.posters.toLocaleString()}
              hint={of}
              qa="channel-activity-posters"
            />
            <Stat
              label="Members who viewed"
              value={activity.viewers.toLocaleString()}
              hint={of}
              qa="channel-activity-viewers"
            />
            <Stat
              label="Last message"
              value={last ? dayLabel(last) : 'None yet'}
              qa="channel-activity-last"
            />
          </div>
        </div>
      )}
    </QueryState>
  );
}
