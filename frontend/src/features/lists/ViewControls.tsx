import { useRef, useState, type ReactNode, type RefObject } from 'react';
import { ArrowUpDown, Filter, Plus, X } from 'lucide-react';
import { IconButton } from '@/shared/components/IconButton/IconButton';
import { Select } from '@/shared/components/Form/Select';
import { TextInput } from '@/shared/components/Form/TextInput';
import { AnchoredPopover } from '@/shared/components/Popover/AnchoredPopover';
import type { ListColumn } from './listModel';
import type { People } from './people';
import {
  MAX_FILTERS,
  MAX_SORTS,
  ME,
  OPS_BY_KIND,
  defaultRule,
  dueThisWeek,
  isComplete,
  onlyMine,
  opLabel,
  sameRule,
  takesValue,
  type FilterOp,
  type FilterRule,
  type SortRule,
  type ViewConfig,
  type ViewKind,
} from './viewModel';

interface Props {
  kind: ViewKind;
  config: Required<ViewConfig>;
  columns: ListColumn[];
  people: People;
  canEdit: boolean;
  onChange: (config: Required<ViewConfig>) => void;
}

type Panel = 'sort' | 'filter';

const family = (op: FilterOp) =>
  op === 'is' || op === 'is_not' ? 'ids' : op === 'lt' || op === 'gt' ? 'compare' : op;

const addButton =
  'inline-flex items-center gap-1.5 rounded-md px-2 py-1 text-sm text-muted transition hover:bg-raised/50 hover:text-fg disabled:cursor-not-allowed disabled:opacity-50 cursor-pointer';

export function ViewControls({ kind, config, columns, people, canEdit, onChange }: Props) {
  const sortRef = useRef<HTMLButtonElement>(null);
  const filterRef = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState<Panel | null>(null);
  const groupable = columns.filter((column) => column.kind === 'select');
  const toggle = (panel: Panel) => setOpen((current) => (current === panel ? null : panel));

  return (
    <div className="flex shrink-0 items-center gap-1">
      {kind === 'board' && (
        <Select
          label="Group by"
          labelHidden
          compact
          value={config.group_by ?? ''}
          disabled={!canEdit || groupable.length === 0}
          onChange={(e) => onChange({ ...config, group_by: e.target.value || null })}
          dataQa="list-group-by"
          className="w-40"
        >
          {!config.group_by && <option value="">Group by…</option>}
          {groupable.map((column) => (
            <option key={column.id} value={column.id}>
              By {column.name}
            </option>
          ))}
        </Select>
      )}
      <ToolbarButton
        anchorRef={sortRef}
        icon={<ArrowUpDown className="h-3.5 w-3.5" />}
        label="Sort"
        count={config.sort.length}
        open={open === 'sort'}
        onClick={() => toggle('sort')}
        qa="list-sort"
      />
      <ToolbarButton
        anchorRef={filterRef}
        icon={<Filter className="h-3.5 w-3.5" />}
        label="Filter"
        count={config.filters.length}
        open={open === 'filter'}
        onClick={() => toggle('filter')}
        qa="list-filter"
      />

      {open === 'sort' && (
        <AnchoredPopover
          anchorRef={sortRef}
          onClose={() => setOpen(null)}
          dataQa="list-sort-panel"
          className="w-96 max-w-[calc(100vw-16px)] bg-surface border border-line rounded-lg shadow-xl p-3"
        >
          <SortPanel
            sort={config.sort}
            columns={columns}
            canEdit={canEdit}
            onChange={(sort) => onChange({ ...config, sort })}
          />
        </AnchoredPopover>
      )}
      {open === 'filter' && (
        <AnchoredPopover
          anchorRef={filterRef}
          onClose={() => setOpen(null)}
          dataQa="list-filter-panel"
          className="w-[30rem] max-w-[calc(100vw-16px)] bg-surface border border-line rounded-lg shadow-xl p-3"
        >
          <FilterPanel
            filters={config.filters}
            columns={columns}
            people={people}
            canEdit={canEdit}
            onChange={(filters) => onChange({ ...config, filters })}
          />
        </AnchoredPopover>
      )}
    </div>
  );
}

function ToolbarButton({
  anchorRef,
  icon,
  label,
  count,
  open,
  onClick,
  qa,
}: {
  anchorRef: RefObject<HTMLButtonElement | null>;
  icon: ReactNode;
  label: string;
  count: number;
  open: boolean;
  onClick: () => void;
  qa: string;
}) {
  return (
    <button
      ref={anchorRef}
      onClick={onClick}
      aria-expanded={open}
      aria-haspopup="dialog"
      data-qa={qa}
      className={`flex items-center gap-1 rounded-md px-2 py-1 text-xs transition cursor-pointer ${
        count > 0 ? 'bg-accent/10 text-accent' : 'text-muted hover:bg-raised/50 hover:text-fg'
      }`}
    >
      {icon}
      {label}
      {count > 0 && <span className="tabular-nums">{count}</span>}
    </button>
  );
}

function SortPanel({
  sort,
  columns,
  canEdit,
  onChange,
}: {
  sort: SortRule[];
  columns: ListColumn[];
  canEdit: boolean;
  onChange: (sort: SortRule[]) => void;
}) {
  const used = new Set(sort.map((rule) => rule.column_id));
  const free = columns.filter((column) => !used.has(column.id));
  const replace = (index: number, rule: SortRule) => onChange(sort.map((r, i) => (i === index ? rule : r)));

  return (
    <fieldset disabled={!canEdit} className="space-y-2" aria-label="Sort">
      {sort.length === 0 && <p className="text-sm text-muted">Items keep the list's own order.</p>}
      {sort.map((rule, index) => (
        <div key={index} className="flex items-center gap-2" data-qa="list-sort-rule">
          <Select
            label={index === 0 ? 'Sort by' : 'Then by'}
            labelHidden
            compact
            data-autofocus={index === 0 || undefined}
            value={rule.column_id}
            onChange={(e) => replace(index, { ...rule, column_id: e.target.value })}
            dataQa="list-sort-column"
            className="flex-1 min-w-0"
          >
            {columns
              .filter((column) => column.id === rule.column_id || !used.has(column.id))
              .map((column) => (
                <option key={column.id} value={column.id}>
                  {column.name}
                </option>
              ))}
          </Select>
          <Select
            label="Direction"
            labelHidden
            compact
            value={rule.direction ?? 'asc'}
            onChange={(e) =>
              replace(index, { ...rule, direction: e.target.value === 'desc' ? 'desc' : 'asc' })
            }
            dataQa="list-sort-direction"
            className="w-36"
          >
            <option value="asc">Ascending</option>
            <option value="desc">Descending</option>
          </Select>
          <IconButton
            label="Remove sort"
            icon={<X className="h-4 w-4" />}
            onClick={() => onChange(sort.filter((_, i) => i !== index))}
            data-qa="list-sort-remove"
          />
        </div>
      ))}
      <button
        onClick={() => onChange([...sort, { column_id: free[0].id, direction: 'asc' }])}
        disabled={sort.length >= MAX_SORTS || free.length === 0}
        data-autofocus={sort.length === 0 || undefined}
        data-qa="list-sort-add"
        className={addButton}
      >
        <Plus className="h-4 w-4" />
        Add sort
      </button>
    </fieldset>
  );
}

function FilterPanel({
  filters,
  columns,
  people,
  canEdit,
  onChange,
}: {
  filters: FilterRule[];
  columns: ListColumn[];
  people: People;
  canEdit: boolean;
  onChange: (filters: FilterRule[]) => void;
}) {
  const [draft, setDraft] = useState<FilterRule[]>(filters);
  const byId = new Map(columns.map((column) => [column.id, column]));
  const presets = [
    { label: 'Only mine', rule: onlyMine(columns), qa: 'list-filter-mine' },
    { label: 'Due this week', rule: dueThisWeek(columns), qa: 'list-filter-week' },
  ].flatMap(({ rule, ...preset }) => (rule ? [{ ...preset, rule }] : []));

  const update = (next: FilterRule[]) => {
    setDraft(next);
    const complete = next.filter(isComplete);
    if (JSON.stringify(complete) !== JSON.stringify(filters)) onChange(complete);
  };
  const replace = (index: number, rule: FilterRule) => update(draft.map((r, i) => (i === index ? rule : r)));

  return (
    <fieldset disabled={!canEdit} className="space-y-3" aria-label="Filter">
      {presets.length > 0 && (
        <div className="flex flex-wrap gap-1.5">
          {presets.map(({ label, rule, qa }) => {
            const on = draft.some((r) => sameRule(r, rule));
            return (
              <button
                key={qa}
                aria-pressed={on}
                onClick={() => update(on ? draft.filter((r) => !sameRule(r, rule)) : [...draft, rule])}
                disabled={!on && draft.length >= MAX_FILTERS}
                data-qa={qa}
                className={`rounded-full border px-2.5 py-0.5 text-xs transition cursor-pointer disabled:cursor-not-allowed ${
                  on
                    ? 'border-accent/60 bg-accent/15 text-accent'
                    : 'border-line text-muted hover:text-fg hover:border-line-strong'
                }`}
              >
                {label}
              </button>
            );
          })}
        </div>
      )}
      {draft.length === 0 && <p className="text-sm text-muted">Every item is shown.</p>}
      {draft.map((rule, index) => {
        const column = byId.get(rule.column_id);
        if (!column) return null;
        return (
          <div
            key={`${index}-${rule.column_id}-${rule.op}`}
            className="grid grid-cols-[repeat(3,minmax(0,1fr))_auto] items-center gap-2"
            data-qa="list-filter-rule"
          >
            <Select
              label="Column"
              labelHidden
              compact
              value={rule.column_id}
              onChange={(e) => {
                const next = byId.get(e.target.value);
                if (next) replace(index, defaultRule(next));
              }}
              dataQa="list-filter-column"
              className="min-w-0"
            >
              {columns.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.name}
                </option>
              ))}
            </Select>
            <Select
              label="Condition"
              labelHidden
              compact
              value={rule.op}
              onChange={(e) => {
                const op = e.target.value as FilterOp;
                const value =
                  family(op) === family(rule.op)
                    ? rule.value
                    : family(op) === 'ids'
                      ? defaultRule(column).value
                      : undefined;
                replace(index, { column_id: rule.column_id, op, value });
              }}
              dataQa="list-filter-op"
              className="min-w-0"
            >
              {OPS_BY_KIND[column.kind].map((op) => (
                <option key={op} value={op}>
                  {opLabel(op, column.kind)}
                </option>
              ))}
            </Select>
            {takesValue(rule.op) && (
              <FilterValue
                rule={rule}
                column={column}
                people={people}
                onChange={(value) => replace(index, { ...rule, value })}
              />
            )}
            <IconButton
              label="Remove filter"
              icon={<X className="h-4 w-4" />}
              className="col-start-4"
              onClick={() => update(draft.filter((_, i) => i !== index))}
              data-qa="list-filter-remove"
            />
          </div>
        );
      })}
      <button
        onClick={() => update([...draft, defaultRule(columns[0])])}
        disabled={columns.length === 0 || draft.length >= MAX_FILTERS}
        data-autofocus
        data-qa="list-filter-add"
        className={addButton}
      >
        <Plus className="h-4 w-4" />
        Add filter
      </button>
    </fieldset>
  );
}

function FilterValue({
  rule,
  column,
  people,
  onChange,
}: {
  rule: FilterRule;
  column: ListColumn;
  people: People;
  onChange: (value: unknown) => void;
}) {
  const initial = typeof rule.value === 'string' || typeof rule.value === 'number' ? String(rule.value) : '';
  const [text, setText] = useState(initial);

  if (rule.op === 'is' || rule.op === 'is_not') {
    const current = Array.isArray(rule.value) && typeof rule.value[0] === 'string' ? rule.value[0] : '';
    const options =
      column.kind === 'person'
        ? [{ id: ME, label: 'Me' }, ...people.eligible.map((p) => ({ id: p.id, label: p.name }))]
        : (column.options.choices ?? []).map((c) => ({ id: c.id, label: c.label }));
    return (
      <Select
        label="Value"
        labelHidden
        compact
        value={current}
        onChange={(e) => onChange(e.target.value ? [e.target.value] : undefined)}
        dataQa="list-filter-value"
        className="min-w-0"
      >
        {current === '' && <option value="">Choose…</option>}
        {options.map((option) => (
          <option key={option.id} value={option.id}>
            {option.label}
          </option>
        ))}
      </Select>
    );
  }

  if (column.kind === 'date') {
    return (
      <TextInput
        label="Date"
        labelHidden
        compact
        type="date"
        value={text}
        onChange={(e) => {
          setText(e.target.value);
          onChange(e.target.value || undefined);
        }}
        dataQa="list-filter-value"
        className="min-w-0"
      />
    );
  }

  const commit = () => {
    const trimmed = text.trim();
    if (column.kind === 'number') {
      const n = Number(trimmed);
      onChange(trimmed !== '' && Number.isFinite(n) ? n : undefined);
    } else {
      onChange(trimmed || undefined);
    }
  };
  return (
    <TextInput
      label="Value"
      labelHidden
      compact
      type={column.kind === 'number' ? 'number' : 'text'}
      value={text}
      placeholder={column.kind === 'number' ? 'Number' : 'Text'}
      autoFocus={initial === ''}
      onChange={(e) => setText(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === 'Enter') commit();
      }}
      dataQa="list-filter-value"
      className="min-w-0"
    />
  );
}
