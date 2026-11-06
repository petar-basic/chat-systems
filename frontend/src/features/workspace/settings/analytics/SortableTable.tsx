import { useState, type ReactNode } from 'react';
import { ArrowDown, ArrowUp } from 'lucide-react';

export interface Column<T> {
  key: string;
  label: string;
  numeric?: boolean;
  value: (row: T) => string | number | null;
  render?: (row: T) => ReactNode;
}

interface Props<T> {
  rows: T[];
  columns: Column<T>[];
  rowKey: (row: T) => string;
  initialSort: string;
  qa: string;
  rowQa: string;
}

function compare(a: string | number | null, b: string | number | null): number {
  if (typeof a === 'number' && typeof b === 'number') return a - b;
  return String(a ?? '').localeCompare(String(b ?? ''));
}

function shown(value: string | number | null): ReactNode {
  if (value === null) return '—';
  return typeof value === 'number' ? value.toLocaleString() : value;
}

export function SortableTable<T>({ rows, columns, rowKey, initialSort, qa, rowQa }: Props<T>) {
  const [sort, setSort] = useState({ key: initialSort, descending: true });
  const column = columns.find((c) => c.key === sort.key) ?? columns[0];
  const first = columns[0];
  const sorted = [...rows].sort((a, b) => {
    const order = compare(column.value(a), column.value(b));
    return (sort.descending ? -order : order) || compare(first.value(a), first.value(b));
  });

  const sortBy = (next: Column<T>) =>
    setSort((current) =>
      current.key === next.key
        ? { key: next.key, descending: !current.descending }
        : { key: next.key, descending: !!next.numeric || next.key.startsWith('last') },
    );

  return (
    <div className="overflow-x-auto rounded-xl border border-line bg-surface" data-qa={qa}>
      <table className="w-full min-w-140">
        <thead>
          <tr className="border-b border-line">
            {columns.map((c) => {
              const active = sort.key === c.key;
              const Arrow = sort.descending ? ArrowDown : ArrowUp;
              return (
                <th
                  key={c.key}
                  scope="col"
                  aria-sort={active ? (sort.descending ? 'descending' : 'ascending') : 'none'}
                  className={`whitespace-nowrap px-3 py-2 text-xs font-medium uppercase tracking-wider text-muted ${c.numeric ? 'text-right' : 'text-left'}`}
                >
                  <button
                    type="button"
                    onClick={() => sortBy(c)}
                    className="inline-flex cursor-pointer items-center gap-1 uppercase hover:text-fg"
                    data-qa={`${qa}-sort-${c.key}`}
                  >
                    {c.label}
                    {active && <Arrow className="h-3 w-3" aria-hidden="true" />}
                  </button>
                </th>
              );
            })}
          </tr>
        </thead>
        <tbody className="divide-y divide-line">
          {sorted.map((row) => (
            <tr key={rowKey(row)} data-qa={rowQa}>
              {columns.map((c) => (
                <td
                  key={c.key}
                  className={`whitespace-nowrap px-3 py-2 text-sm ${c.numeric ? 'text-right tabular-nums text-fg-soft' : 'text-fg'}`}
                >
                  {c.render ? c.render(row) : shown(c.value(row))}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
