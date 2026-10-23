import { useMemo, useRef, useState, type ReactNode } from 'react';
import { ChevronDown, Hash, Lock, MessageSquare, X } from 'lucide-react';
import { AnchoredPopover } from '@/shared/components/Popover/AnchoredPopover';
import { Avatar } from '@/shared/components/Avatar/Avatar';
import { displayNameOf } from '@/lib/userHelpers';
import {
  HAS_LABELS,
  dateLabel,
  dayKey,
  memberName,
  placeName,
  setDate,
  shiftDay,
  type FilterContext,
  type HasFilter,
  type SearchFilters,
} from './searchFilters';

type Picker = 'from' | 'in' | 'has' | 'date';

interface Props {
  filters: SearchFilters;
  onChange: (filters: SearchFilters) => void;
  context: FilterContext;
}

const LIST_LIMIT = 8;

function Chip({
  label,
  value,
  open,
  onOpen,
  onClear,
  anchorRef,
  qa,
}: {
  label: string;
  value: string | null;
  open: boolean;
  onOpen: () => void;
  onClear: () => void;
  anchorRef: React.RefObject<HTMLButtonElement | null>;
  qa: string;
}) {
  return (
    <span
      className={`inline-flex items-center rounded-full border text-xs transition ${
        value ? 'border-purple-500/50 bg-purple-600/15 text-fg' : 'border-line text-fg-dim hover:bg-raised/40'
      }`}
    >
      <button
        ref={anchorRef}
        type="button"
        onClick={onOpen}
        aria-expanded={open}
        aria-haspopup="dialog"
        data-qa={qa}
        className="inline-flex items-center gap-1 py-1 pl-2.5 pr-2 cursor-pointer"
      >
        <span className={value ? 'text-muted' : undefined}>{label}</span>
        {value ? (
          <span className="max-w-32 truncate font-medium">{value}</span>
        ) : (
          <ChevronDown className="h-3 w-3" />
        )}
      </button>
      {value && (
        <button
          type="button"
          onClick={onClear}
          aria-label={`Remove the ${label.toLowerCase()} filter`}
          className="-ml-1 mr-1 rounded-full p-0.5 text-muted hover:bg-raised hover:text-fg cursor-pointer"
        >
          <X className="h-3 w-3" />
        </button>
      )}
    </span>
  );
}

function Option({ onSelect, children }: { onSelect: () => void; children: ReactNode }) {
  return (
    <button
      type="button"
      onClick={onSelect}
      className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm text-fg-dim hover:bg-raised cursor-pointer"
    >
      {children}
    </button>
  );
}

function FilterInput({
  value,
  onChange,
  label,
}: {
  value: string;
  onChange: (v: string) => void;
  label: string;
}) {
  return (
    <input
      autoFocus
      value={value}
      onChange={(e) => onChange(e.target.value)}
      placeholder={label}
      aria-label={label}
      className="mx-2 mb-1 w-[calc(100%-1rem)] rounded-md border border-line bg-app px-2 py-1 text-sm text-fg placeholder-subtle focus:outline-none"
    />
  );
}

function PeopleList({ ctx, onPick }: { ctx: FilterContext; onPick: (userId: string) => void }) {
  const [term, setTerm] = useState('');
  const people = useMemo(() => {
    const wanted = term.trim().toLowerCase();
    return ctx.members
      .filter((m) => displayNameOf(m.display_name).toLowerCase().includes(wanted))
      .slice(0, LIST_LIMIT);
  }, [ctx.members, term]);
  return (
    <>
      <FilterInput value={term} onChange={setTerm} label="Find a person" />
      {people.map((m) => (
        <Option key={m.user_id} onSelect={() => onPick(m.user_id)}>
          <Avatar userId={m.user_id} name={displayNameOf(m.display_name)} size="xs" />
          <span className="truncate">{memberName(ctx, m.user_id)}</span>
        </Option>
      ))}
      {people.length === 0 && <p className="px-3 py-1.5 text-sm text-muted">Nobody matches</p>}
    </>
  );
}

function PlaceList({ ctx, onPick }: { ctx: FilterContext; onPick: (placeId: string) => void }) {
  const [term, setTerm] = useState('');
  const places = useMemo(() => {
    const wanted = term.trim().toLowerCase();
    const all = [
      ...ctx.channels.map((c) => ({ id: c.id, kind: c.channel_type ?? 'public' })),
      ...ctx.conversations.map((c) => ({ id: c.id, kind: 'dm' })),
    ];
    return all
      .map((place) => ({ ...place, name: placeName(ctx, place.id) }))
      .filter((place) => place.name.toLowerCase().includes(wanted))
      .slice(0, LIST_LIMIT);
  }, [ctx, term]);
  return (
    <>
      <FilterInput value={term} onChange={setTerm} label="Find a channel or conversation" />
      {places.map((place) => (
        <Option key={place.id} onSelect={() => onPick(place.id)}>
          {place.kind === 'dm' ? (
            <MessageSquare className="h-3.5 w-3.5 shrink-0 text-muted" />
          ) : place.kind === 'private' ? (
            <Lock className="h-3.5 w-3.5 shrink-0 text-muted" />
          ) : (
            <Hash className="h-3.5 w-3.5 shrink-0 text-muted" />
          )}
          <span className="truncate">{place.kind === 'dm' ? place.name : place.name.slice(1)}</span>
        </Option>
      ))}
      {places.length === 0 && <p className="px-3 py-1.5 text-sm text-muted">Nothing matches</p>}
    </>
  );
}

type DateFilter = Pick<SearchFilters, 'on' | 'after' | 'before'>;

function DateChoices({
  filters,
  today,
  onPreset,
  onCustom,
}: {
  filters: SearchFilters;
  today: Date;
  onPreset: (date: DateFilter) => void;
  onCustom: (date: DateFilter) => void;
}) {
  const now = dayKey(today);
  const presets = [
    { label: 'Today', date: { on: now } },
    { label: 'Yesterday', date: { on: shiftDay(now, -1) } },
    { label: 'Last 7 days', date: { after: shiftDay(now, -7) } },
    { label: 'Last 30 days', date: { after: shiftDay(now, -30) } },
  ];
  const fields = [
    { key: 'on', label: 'On' },
    { key: 'after', label: 'After' },
    { key: 'before', label: 'Before' },
  ] as const;
  return (
    <>
      {presets.map((preset) => (
        <Option key={preset.label} onSelect={() => onPreset(preset.date)}>
          {preset.label}
        </Option>
      ))}
      <div className="mt-1 space-y-1.5 border-t border-line/60 px-3 pb-1 pt-2">
        {fields.map((field) => (
          <label key={field.key} className="flex items-center justify-between gap-2 text-xs text-muted">
            {field.label}
            <input
              type="date"
              value={filters[field.key] ?? ''}
              max={now}
              data-qa={`search-filter-date-${field.key}`}
              onChange={(e) => {
                const day = e.target.value || undefined;
                onCustom(
                  field.key === 'on'
                    ? { on: day }
                    : { after: filters.after, before: filters.before, [field.key]: day },
                );
              }}
              className="rounded-md border border-line bg-app px-1.5 py-0.5 text-xs text-fg focus:outline-none"
            />
          </label>
        ))}
      </div>
    </>
  );
}

export function SearchFilterBar({ filters, onChange, context }: Props) {
  const [open, setOpen] = useState<Picker | null>(null);
  const anchors = {
    from: useRef<HTMLButtonElement>(null),
    in: useRef<HTMLButtonElement>(null),
    has: useRef<HTMLButtonElement>(null),
    date: useRef<HTMLButtonElement>(null),
  };
  const toggle = (picker: Picker) => setOpen((current) => (current === picker ? null : picker));
  const pick = (next: SearchFilters) => {
    onChange(next);
    setOpen(null);
  };

  const chips: { picker: Picker; label: string; value: string | null; clear: SearchFilters }[] = [
    {
      picker: 'from',
      label: 'From',
      value: filters.from ? memberName(context, filters.from) : null,
      clear: { ...filters, from: undefined },
    },
    {
      picker: 'in',
      label: 'In',
      value: filters.in ? placeName(context, filters.in) : null,
      clear: { ...filters, in: undefined },
    },
    {
      picker: 'has',
      label: 'Has',
      value: filters.has ? HAS_LABELS[filters.has] : null,
      clear: { ...filters, has: undefined },
    },
    {
      picker: 'date',
      label: 'Date',
      value: dateLabel(filters),
      clear: setDate(filters, { after: undefined, before: undefined }),
    },
  ];

  return (
    <div className="flex flex-wrap gap-1.5" data-qa="search-filters">
      {chips.map((chip) => (
        <Chip
          key={chip.picker}
          label={chip.label}
          value={chip.value}
          open={open === chip.picker}
          onOpen={() => toggle(chip.picker)}
          onClear={() => onChange(chip.clear)}
          anchorRef={anchors[chip.picker]}
          qa={`search-filter-${chip.picker}`}
        />
      ))}
      {open && (
        <AnchoredPopover
          anchorRef={anchors[open]}
          onClose={() => setOpen(null)}
          align="start"
          dataQa={`search-filter-${open}-menu`}
          className="w-60 rounded-lg border border-line bg-surface py-2 shadow-xl"
        >
          {open === 'from' && <PeopleList ctx={context} onPick={(from) => pick({ ...filters, from })} />}
          {open === 'in' && <PlaceList ctx={context} onPick={(place) => pick({ ...filters, in: place })} />}
          {open === 'has' &&
            (Object.keys(HAS_LABELS) as HasFilter[]).map((has) => (
              <Option key={has} onSelect={() => pick({ ...filters, has })}>
                {HAS_LABELS[has]}
              </Option>
            ))}
          {open === 'date' && (
            <DateChoices
              filters={filters}
              today={context.today}
              onPreset={(date) => pick(setDate(filters, date))}
              onCustom={(date) => onChange(setDate(filters, date))}
            />
          )}
        </AnchoredPopover>
      )}
    </div>
  );
}
