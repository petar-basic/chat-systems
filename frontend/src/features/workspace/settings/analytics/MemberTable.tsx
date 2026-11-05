import { useState } from 'react';
import { ArrowDown, ArrowUp } from 'lucide-react';
import { dayLabel } from './days';

export interface MemberRow {
  userId: string;
  name: string;
  role: string;
  daysActive: number;
  messages: number;
  lastActive: string | null;
}

type SortKey = 'name' | 'role' | 'daysActive' | 'messages' | 'lastActive';

const COLUMNS: { key: SortKey; label: string; numeric?: boolean }[] = [
  { key: 'name', label: 'Member' },
  { key: 'role', label: 'Role' },
  { key: 'daysActive', label: 'Days active', numeric: true },
  { key: 'messages', label: 'Messages', numeric: true },
  { key: 'lastActive', label: 'Last active' },
];

function compare(a: MemberRow, b: MemberRow, key: SortKey): number {
  switch (key) {
    case 'name':
    case 'role':
      return a[key].localeCompare(b[key]);
    case 'daysActive':
    case 'messages':
      return a[key] - b[key];
    case 'lastActive':
      return (a.lastActive ?? '').localeCompare(b.lastActive ?? '');
  }
}

function sortRows(rows: MemberRow[], key: SortKey, descending: boolean): MemberRow[] {
  return [...rows].sort((a, b) => {
    const order = compare(a, b, key);
    return (descending ? -order : order) || a.name.localeCompare(b.name);
  });
}

export function MemberTable({ rows }: { rows: MemberRow[] }) {
  const [sort, setSort] = useState<{ key: SortKey; descending: boolean }>({
    key: 'daysActive',
    descending: true,
  });
  const sorted = sortRows(rows, sort.key, sort.descending);

  const sortBy = (key: SortKey) =>
    setSort((current) =>
      current.key === key
        ? { key, descending: !current.descending }
        : { key, descending: key !== 'name' && key !== 'role' },
    );

  return (
    <div className="overflow-x-auto rounded-xl border border-line bg-surface" data-qa="analytics-members">
      <table className="w-full min-w-140">
        <thead>
          <tr className="border-b border-line">
            {COLUMNS.map((column) => {
              const active = sort.key === column.key;
              const Arrow = sort.descending ? ArrowDown : ArrowUp;
              return (
                <th
                  key={column.key}
                  scope="col"
                  aria-sort={active ? (sort.descending ? 'descending' : 'ascending') : 'none'}
                  className={`px-4 py-2 text-xs font-medium uppercase tracking-wider text-muted ${column.numeric ? 'text-right' : 'text-left'}`}
                >
                  <button
                    type="button"
                    onClick={() => sortBy(column.key)}
                    className="inline-flex cursor-pointer items-center gap-1 uppercase hover:text-fg"
                    data-qa={`analytics-sort-${column.key}`}
                  >
                    {column.label}
                    {active && <Arrow className="h-3 w-3" aria-hidden="true" />}
                  </button>
                </th>
              );
            })}
          </tr>
        </thead>
        <tbody className="divide-y divide-line">
          {sorted.map((row) => (
            <tr key={row.userId} data-qa="analytics-member-row">
              <td className="px-4 py-2 text-sm text-fg">{row.name}</td>
              <td className="px-4 py-2 text-sm capitalize text-fg-soft">{row.role}</td>
              <td className="px-4 py-2 text-right text-sm tabular-nums text-fg-soft">
                {row.daysActive.toLocaleString()}
              </td>
              <td className="px-4 py-2 text-right text-sm tabular-nums text-fg-soft">
                {row.messages.toLocaleString()}
              </td>
              <td className="px-4 py-2 text-sm text-muted">
                {row.lastActive ? dayLabel(row.lastActive) : 'Never'}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
