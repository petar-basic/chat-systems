import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';
import type { MemberActivity } from '@/features/workspace/hooks/useAnalytics';
import { dayLabel } from './days';
import { SortableTable, type Column } from './SortableTable';

export interface MemberRow {
  userId: string;
  name: string;
  role: MemberActivity['role'];
  daysActive: number;
  messages: number;
  lastActive: string | null;
}

function columns(t: TFunction<'workspace'>): Column<MemberRow>[] {
  return [
    { key: 'name', label: t('settings.analytics.members.member'), value: (r) => r.name },
    {
      key: 'role',
      label: t('settings.analytics.members.role'),
      value: (r) => r.role,
      render: (r) => <span className="text-fg-soft">{t(`settings.members.roles.${r.role}`)}</span>,
    },
    {
      key: 'daysActive',
      label: t('settings.analytics.members.daysActive'),
      numeric: true,
      value: (r) => r.daysActive,
    },
    {
      key: 'messages',
      label: t('settings.analytics.members.messages'),
      numeric: true,
      value: (r) => r.messages,
    },
    {
      key: 'lastActive',
      label: t('settings.analytics.members.lastActive'),
      value: (r) => r.lastActive,
      render: (r) => (
        <span className="text-muted">
          {r.lastActive ? dayLabel(r.lastActive) : t('settings.analytics.members.never')}
        </span>
      ),
    },
  ];
}

export function MemberTable({ rows }: { rows: MemberRow[] }) {
  const { t } = useTranslation('workspace');
  return (
    <SortableTable
      rows={rows}
      columns={columns(t)}
      rowKey={(r) => r.userId}
      initialSort="daysActive"
      qa="analytics-members"
      rowQa="analytics-member-row"
    />
  );
}
