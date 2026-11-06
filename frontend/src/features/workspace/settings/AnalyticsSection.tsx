import { useState } from 'react';
import { Download } from 'lucide-react';
import { Button } from '@/shared/components/Button/Button';
import { Select } from '@/shared/components/Form/Select';
import { Checkbox } from '@/shared/components/Form/Checkbox';
import { QueryState } from '@/shared/components/QueryState/QueryState';
import { displayNameOf } from '@/lib/userHelpers';
import { downloadCsv } from '@/lib/csv';
import type { WorkspaceMember } from '@/stores/workspace';
import {
  ANALYTICS_WINDOWS,
  useChannelActivity,
  useMemberAnalytics,
  useWorkspaceAnalytics,
  type AnalyticsWindow,
  type ChannelActivity,
  type DailyActivity,
  type MemberActivity,
  type WorkspaceAnalytics,
} from '@/features/workspace/hooks/useAnalytics';
import { DayChart } from './analytics/DayChart';
import { MemberTable, type MemberRow } from './analytics/MemberTable';
import { ChannelTable } from './analytics/ChannelTable';
import { QuietChannels } from './analytics/QuietChannels';
import { dayLabel, dayOf } from './analytics/days';

interface Props {
  workspaceId: string;
  workspaceName: string;
  instanceUrl?: string;
  members: WorkspaceMember[];
}

function sum(daily: DailyActivity[], pick: (d: DailyActivity) => number) {
  return daily.reduce((total, d) => total + pick(d), 0);
}

function messagesOf(d: DailyActivity) {
  return d.public_messages + d.private_messages + d.direct_messages;
}

function Stat({ label, value, hint }: { label: string; value: number; hint?: string }) {
  return (
    <div className="rounded-xl border border-line bg-surface px-4 py-3">
      <div className="text-xs text-muted">{label}</div>
      <div className="text-2xl font-semibold tabular-nums text-fg">{value.toLocaleString()}</div>
      {hint && <div className="text-xs text-muted">{hint}</div>}
    </div>
  );
}

function CsvButton({ onClick, qa }: { onClick: () => void; qa: string }) {
  return (
    <Button size="sm" onClick={onClick} icon={<Download className="h-3.5 w-3.5" />} data-qa={qa}>
      Export CSV
    </Button>
  );
}

function fileName(workspaceName: string, what: string, days: AnalyticsWindow) {
  const slug =
    workspaceName
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-|-$/g, '') || 'workspace';
  return `${slug}-${what}-${days}-days.csv`;
}

function Overview({ report }: { report: WorkspaceAnalytics }) {
  const { daily } = report;
  const days = daily.map((d) => d.day);
  return (
    <div className="space-y-6">
      <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
        <Stat label="Members" value={report.members} />
        <Stat
          label="Active in these days"
          value={report.active_members}
          hint={`of ${report.members.toLocaleString()} members`}
        />
        <Stat label="Messages" value={sum(daily, messagesOf)} />
        <Stat label="Joined" value={sum(daily, (d) => d.joined)} />
      </div>

      <DayChart
        title="Active members per day"
        days={days}
        ceiling={report.members}
        series={[
          {
            label: 'Active',
            fill: 'fill-accent',
            swatch: 'bg-accent',
            values: daily.map((d) => d.active_members),
          },
        ]}
        qa="analytics-active-chart"
      />

      {report.weekly.length > 0 && (
        <div className="space-y-2">
          <h4 className="text-sm font-medium text-fg">Active members per week</h4>
          <ul className="flex flex-wrap gap-2" data-qa="analytics-weeks">
            {report.weekly.map((week) => (
              <li key={week.week} className="rounded-lg border border-line px-3 py-1.5 text-xs text-fg-soft">
                Week of {dayLabel(week.week)}:{' '}
                <span className="font-medium text-fg">{week.active_members.toLocaleString()}</span>
              </li>
            ))}
          </ul>
        </div>
      )}

      <DayChart
        title="Messages per day"
        days={days}
        series={[
          {
            label: 'Public channels',
            fill: 'fill-accent',
            swatch: 'bg-accent',
            values: daily.map((d) => d.public_messages),
          },
          {
            label: 'Private channels',
            fill: 'fill-info',
            swatch: 'bg-info',
            values: daily.map((d) => d.private_messages),
          },
          {
            label: 'Direct messages',
            fill: 'fill-warning',
            swatch: 'bg-warning',
            values: daily.map((d) => d.direct_messages),
          },
        ]}
        qa="analytics-messages-chart"
      />
    </div>
  );
}

function dailyCsv(daily: DailyActivity[]) {
  return [
    [
      'Day',
      'Members',
      'Active members',
      'Joined',
      'Public channel messages',
      'Private channel messages',
      'Direct messages',
      'Files',
      'Reactions',
    ],
    ...daily.map((d) => [
      d.day,
      d.members,
      d.active_members,
      d.joined,
      d.public_messages,
      d.private_messages,
      d.direct_messages,
      d.files,
      d.reactions,
    ]),
  ];
}

function memberRows(activity: MemberActivity[], members: WorkspaceMember[]): MemberRow[] {
  const byId = new Map(members.map((m) => [m.user_id, m]));
  return activity.map((a) => {
    const member = byId.get(a.user_id);
    return {
      userId: a.user_id,
      name: displayNameOf(member?.display_name || member?.email),
      role: a.role,
      daysActive: a.days_active,
      messages: a.messages,
      lastActive: a.last_active ?? null,
    };
  });
}

function membersCsv(rows: MemberRow[]) {
  return [
    ['Member', 'Role', 'Days active', 'Messages', 'Last active'],
    ...rows.map((r) => [r.name, r.role, r.daysActive, r.messages, r.lastActive]),
  ];
}

function channelsCsv(rows: ChannelActivity[], guests: boolean) {
  return [
    [
      'Channel',
      'Type',
      'Archived',
      'Created',
      'Members',
      ...(guests ? ['Guests'] : []),
      'Messages',
      'Members who posted',
      'Members who viewed',
      'Reactions',
      'Last message',
    ],
    ...rows.map((c) => [
      c.name,
      c.channel_type,
      c.is_archived ? 'yes' : 'no',
      dayOf(c.created_at),
      c.members,
      ...(guests ? [c.guests] : []),
      c.messages,
      c.posters,
      c.viewers,
      c.reactions,
      dayOf(c.last_message_at),
    ]),
  ];
}

type Tab = 'overview' | 'channels' | 'quiet';

const TABS: { key: Tab; label: string }[] = [
  { key: 'overview', label: 'Overview' },
  { key: 'channels', label: 'Channels' },
  { key: 'quiet', label: 'Quiet channels' },
];

interface Scope {
  workspaceId: string;
  workspaceName: string;
  instanceUrl?: string;
  days: AnalyticsWindow;
  guests: boolean;
}

function WorkspaceTab({
  workspaceId,
  workspaceName,
  instanceUrl,
  days,
  guests,
  members,
}: Scope & {
  members: WorkspaceMember[];
}) {
  const report = useWorkspaceAnalytics(workspaceId, days, guests, instanceUrl);
  const activity = useMemberAnalytics(workspaceId, days, guests, instanceUrl);
  const rows = memberRows(activity.data ?? [], members);

  return (
    <>
      <QueryState isLoading={report.isLoading} isError={report.isError} onRetry={() => void report.refetch()}>
        {report.data && (
          <section className="space-y-3">
            <div className="flex items-center justify-between gap-2">
              <h3 className="text-base font-semibold text-fg">
                {dayLabel(report.data.from)} – {dayLabel(report.data.to)}
              </h3>
              <CsvButton
                onClick={() =>
                  downloadCsv(fileName(workspaceName, 'days', days), dailyCsv(report.data.daily))
                }
                qa="analytics-days-csv"
              />
            </div>
            <Overview report={report.data} />
          </section>
        )}
      </QueryState>

      <QueryState
        isLoading={activity.isLoading}
        isError={activity.isError}
        onRetry={() => void activity.refetch()}
      >
        <section className="space-y-3">
          <div className="flex items-center justify-between gap-2">
            <h3 className="text-base font-semibold text-fg">Members</h3>
            <CsvButton
              onClick={() => downloadCsv(fileName(workspaceName, 'members', days), membersCsv(rows))}
              qa="analytics-members-csv"
            />
          </div>
          <MemberTable rows={rows} />
        </section>
      </QueryState>
    </>
  );
}

function ChannelsTab({ workspaceId, workspaceName, instanceUrl, days, guests }: Scope) {
  const channels = useChannelActivity(workspaceId, days, guests, instanceUrl);
  const rows = channels.data ?? [];

  return (
    <QueryState
      isLoading={channels.isLoading}
      isError={channels.isError}
      onRetry={() => void channels.refetch()}
    >
      <section className="space-y-3">
        <div className="flex items-center justify-between gap-2">
          <h3 className="text-base font-semibold text-fg">Channels</h3>
          <CsvButton
            onClick={() => downloadCsv(fileName(workspaceName, 'channels', days), channelsCsv(rows, guests))}
            qa="analytics-channels-csv"
          />
        </div>
        <ChannelTable rows={rows} guests={guests} />
      </section>
    </QueryState>
  );
}

export function AnalyticsSection({ workspaceId, workspaceName, instanceUrl, members }: Props) {
  const [tab, setTab] = useState<Tab>('overview');
  const [days, setDays] = useState<AnalyticsWindow>(30);
  const [guests, setGuests] = useState(true);
  const scope = { workspaceId, workspaceName, instanceUrl, days, guests };

  return (
    <div className="space-y-6" data-qa="analytics-panel">
      <div className="flex gap-1" role="tablist" aria-label="Analytics">
        {TABS.map((t) => (
          <button
            key={t.key}
            type="button"
            role="tab"
            aria-selected={tab === t.key}
            onClick={() => setTab(t.key)}
            className={`cursor-pointer rounded-full px-3 py-1 text-sm transition ${
              tab === t.key ? 'bg-accent text-white' : 'text-fg-dim hover:bg-raised'
            }`}
            data-qa={`analytics-tab-${t.key}`}
          >
            {t.label}
          </button>
        ))}
      </div>

      {tab === 'quiet' ? (
        <QuietChannels workspaceId={workspaceId} instanceUrl={instanceUrl} />
      ) : (
        <>
          <div className="flex flex-wrap items-end gap-4">
            <Select
              label="Period"
              value={days}
              onChange={(e) => setDays(Number(e.target.value) as AnalyticsWindow)}
              dataQa="analytics-window"
              className="w-48"
            >
              {ANALYTICS_WINDOWS.map((n) => (
                <option key={n} value={n}>
                  Last {n} days
                </option>
              ))}
            </Select>
            <Checkbox
              label="Include guests"
              checked={guests}
              onChange={setGuests}
              dataQa="analytics-guests"
            />
          </div>
          <p className="text-xs text-muted">
            Counts only, never what anyone wrote. A member is active on a day they post, react or read a
            conversation. Days run midnight to midnight UTC.
          </p>
          {tab === 'overview' ? <WorkspaceTab {...scope} members={members} /> : <ChannelsTab {...scope} />}
        </>
      )}
    </div>
  );
}
