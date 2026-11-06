import { dayLabel } from './days';
import { SortableTable, type Column } from './SortableTable';

export interface MemberRow {
  userId: string;
  name: string;
  role: string;
  daysActive: number;
  messages: number;
  lastActive: string | null;
}

const COLUMNS: Column<MemberRow>[] = [
  { key: 'name', label: 'Member', value: (r) => r.name },
  {
    key: 'role',
    label: 'Role',
    value: (r) => r.role,
    render: (r) => <span className="capitalize text-fg-soft">{r.role}</span>,
  },
  { key: 'daysActive', label: 'Days active', numeric: true, value: (r) => r.daysActive },
  { key: 'messages', label: 'Messages', numeric: true, value: (r) => r.messages },
  {
    key: 'lastActive',
    label: 'Last active',
    value: (r) => r.lastActive,
    render: (r) => <span className="text-muted">{r.lastActive ? dayLabel(r.lastActive) : 'Never'}</span>,
  },
];

export function MemberTable({ rows }: { rows: MemberRow[] }) {
  return (
    <SortableTable
      rows={rows}
      columns={COLUMNS}
      rowKey={(r) => r.userId}
      initialSort="daysActive"
      qa="analytics-members"
      rowQa="analytics-member-row"
    />
  );
}
