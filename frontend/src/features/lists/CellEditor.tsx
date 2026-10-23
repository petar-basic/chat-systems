import { useId, useRef, useState, type KeyboardEvent, type RefObject } from 'react';
import { Check, UserX } from 'lucide-react';
import { TextInput } from '@/shared/components/Form/TextInput';
import { Select } from '@/shared/components/Form/Select';
import { AnchoredPopover } from '@/shared/components/Popover/AnchoredPopover';
import { Avatar } from '@/shared/components/Avatar/Avatar';
import { toast } from '@/shared/components/Toast';
import { ChoiceChip } from './CellContent';
import { choiceOf, draftOf, parseDraft, type ListColumn } from './listModel';
import type { People } from './people';

export type Move = 'right' | 'left' | 'down' | null;

interface EditorProps {
  column: ListColumn;
  value: unknown;
  onCommit: (value: unknown, move: Move) => void;
  onCancel: () => void;
}

interface Props extends EditorProps {
  initialDraft: string | null;
  people: People;
  anchorRef: RefObject<HTMLElement | null>;
}

const POPOVER = 'w-64 bg-surface border border-line rounded-lg shadow-xl p-1';

function cancelOnEscape(e: KeyboardEvent, onCancel: () => void) {
  if (e.key !== 'Escape') return false;
  e.preventDefault();
  e.stopPropagation();
  onCancel();
  return true;
}

function TextEditor({
  column,
  value,
  initialDraft,
  onCommit,
  onCancel,
}: EditorProps & { initialDraft: string | null }) {
  const [draft, setDraft] = useState(initialDraft ?? draftOf(column, value));
  const [error, setError] = useState<string | null>(null);
  const done = useRef(false);

  const finish = (move: Move) => {
    if (done.current) return;
    const parsed = parseDraft(column.kind, draft);
    if ('error' in parsed) {
      setError(parsed.error);
      return;
    }
    done.current = true;
    onCommit(parsed.value, move);
  };

  return (
    <TextInput
      label={column.name}
      labelHidden
      compact
      autoFocus
      type={column.kind === 'date' ? 'date' : 'text'}
      inputMode={column.kind === 'number' ? 'decimal' : undefined}
      value={draft}
      error={error}
      dataQa="cell-editor"
      className="w-full"
      onChange={(e) => {
        setDraft(e.target.value);
        setError(null);
      }}
      onKeyDown={(e) => {
        if (
          cancelOnEscape(e, () => {
            done.current = true;
            onCancel();
          })
        )
          return;
        if (e.key === 'Enter') {
          e.preventDefault();
          finish('down');
        } else if (e.key === 'Tab') {
          e.preventDefault();
          finish(e.shiftKey ? 'left' : 'right');
        }
      }}
      onBlur={() => {
        if (done.current) return;
        const parsed = parseDraft(column.kind, draft);
        done.current = true;
        if ('error' in parsed) {
          toast.error(`${column.name}: ${parsed.error}`);
          onCancel();
        } else {
          onCommit(parsed.value, null);
        }
      }}
    />
  );
}

function SelectEditor({ column, value, onCommit, onCancel }: EditorProps) {
  const current = choiceOf(column, value)?.id ?? '';
  const done = useRef(false);
  const commit = (next: string, move: Move) => {
    if (done.current) return;
    done.current = true;
    onCommit(next === '' ? null : next, move);
  };
  return (
    <Select
      label={column.name}
      labelHidden
      compact
      autoFocus
      value={current}
      dataQa="cell-editor"
      className="w-full"
      onChange={(e) => commit(e.target.value, null)}
      onKeyDown={(e) => {
        if (
          cancelOnEscape(e, () => {
            done.current = true;
            onCancel();
          })
        )
          return;
        if (e.key === 'Tab') {
          e.preventDefault();
          commit(current, e.shiftKey ? 'left' : 'right');
        }
      }}
      onBlur={() => {
        if (done.current) return;
        done.current = true;
        onCancel();
      }}
    >
      <option value="">—</option>
      {column.options.choices?.map((choice) => (
        <option key={choice.id} value={choice.id}>
          {choice.label}
        </option>
      ))}
    </Select>
  );
}

function MultiSelectEditor({
  column,
  value,
  onCommit,
  onCancel,
  anchorRef,
}: EditorProps & {
  anchorRef: RefObject<HTMLElement | null>;
}) {
  const choices = column.options.choices ?? [];
  const [chosen, setChosen] = useState<string[]>(() =>
    Array.isArray(value) ? value.filter((id): id is string => typeof id === 'string') : [],
  );
  const [active, setActive] = useState(0);
  const idPrefix = useId();
  const toggle = (id: string) =>
    setChosen((current) => (current.includes(id) ? current.filter((c) => c !== id) : [...current, id]));
  const commit = (move: Move) => onCommit(chosen.length === 0 ? null : chosen, move);

  return (
    <AnchoredPopover anchorRef={anchorRef} onClose={() => commit(null)} align="start" className={POPOVER}>
      <div
        role="listbox"
        aria-label={column.name}
        aria-multiselectable
        tabIndex={0}
        data-autofocus
        data-qa="cell-editor"
        aria-activedescendant={choices[active] ? `${idPrefix}-${choices[active].id}` : undefined}
        className="max-h-64 overflow-y-auto focus:outline-none"
        onKeyDown={(e) => {
          if (cancelOnEscape(e, onCancel)) return;
          if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
            e.preventDefault();
            const step = e.key === 'ArrowDown' ? 1 : -1;
            setActive((i) => Math.min(choices.length - 1, Math.max(0, i + step)));
          } else if (e.key === ' ') {
            e.preventDefault();
            if (choices[active]) toggle(choices[active].id);
          } else if (e.key === 'Enter') {
            e.preventDefault();
            commit('down');
          } else if (e.key === 'Tab') {
            e.preventDefault();
            commit(e.shiftKey ? 'left' : 'right');
          }
        }}
      >
        {choices.length === 0 && (
          <p className="px-2 py-1.5 text-sm text-muted">This column has no choices yet.</p>
        )}
        {choices.map((choice, index) => {
          const selected = chosen.includes(choice.id);
          return (
            <div
              key={choice.id}
              id={`${idPrefix}-${choice.id}`}
              role="option"
              aria-selected={selected}
              data-qa="cell-editor-choice"
              onMouseDown={(e) => e.preventDefault()}
              onClick={() => {
                setActive(index);
                toggle(choice.id);
              }}
              className={`flex cursor-pointer items-center gap-2 rounded px-2 py-1.5 ${index === active ? 'bg-raised' : ''}`}
            >
              <span className="flex h-4 w-4 shrink-0 items-center justify-center text-accent-soft">
                {selected && <Check className="h-4 w-4" />}
              </span>
              <ChoiceChip choice={choice} />
            </div>
          );
        })}
      </div>
    </AnchoredPopover>
  );
}

function PersonEditor({
  column,
  value,
  onCommit,
  onCancel,
  anchorRef,
  people,
  initialDraft,
}: EditorProps & {
  anchorRef: RefObject<HTMLElement | null>;
  people: People;
  initialDraft: string | null;
}) {
  const [query, setQuery] = useState(initialDraft ?? '');
  const [active, setActive] = useState(0);
  const idPrefix = useId();
  const needle = query.trim().toLowerCase();
  const matches = people.eligible.filter((person) => person.name.toLowerCase().includes(needle));
  const options = [...matches.map((person) => ({ id: person.id, person })), { id: '', person: null }];
  const pick = (index: number, move: Move) => {
    const option = options[index];
    if (option) onCommit(option.id === '' ? null : option.id, move);
  };

  return (
    <AnchoredPopover anchorRef={anchorRef} onClose={onCancel} align="start" className={POPOVER}>
      <TextInput
        label={`Pick a person for ${column.name}`}
        labelHidden
        compact
        value={query}
        placeholder="Find a person"
        dataQa="cell-editor"
        role="combobox"
        aria-expanded
        aria-controls={`${idPrefix}-options`}
        aria-activedescendant={options[active] ? `${idPrefix}-${options[active].id || 'nobody'}` : undefined}
        data-autofocus
        onChange={(e) => {
          setQuery(e.target.value);
          setActive(0);
        }}
        onKeyDown={(e) => {
          if (cancelOnEscape(e, onCancel)) return;
          if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
            e.preventDefault();
            const step = e.key === 'ArrowDown' ? 1 : -1;
            setActive((i) => Math.min(options.length - 1, Math.max(0, i + step)));
          } else if (e.key === 'Enter') {
            e.preventDefault();
            pick(active, 'down');
          } else if (e.key === 'Tab') {
            e.preventDefault();
            onCommit(value ?? null, e.shiftKey ? 'left' : 'right');
          }
        }}
      />
      <div
        id={`${idPrefix}-options`}
        role="listbox"
        aria-label={column.name}
        className="mt-1 max-h-60 overflow-y-auto"
      >
        {options.map((option, index) => (
          <div
            key={option.id || 'nobody'}
            id={`${idPrefix}-${option.id || 'nobody'}`}
            role="option"
            aria-selected={option.id === (value ?? '')}
            data-qa="cell-editor-person"
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => pick(index, null)}
            className={`flex cursor-pointer items-center gap-2 rounded px-2 py-1.5 text-sm text-fg-dim ${
              index === active ? 'bg-raised' : ''
            }`}
          >
            {option.person ? (
              <>
                <Avatar
                  userId={option.person.id}
                  name={option.person.name}
                  avatarUrl={option.person.avatarUrl}
                  size="xs"
                />
                <span className="truncate">{option.person.name}</span>
              </>
            ) : (
              <>
                <UserX className="h-4 w-4 text-muted" />
                <span className="text-muted">Nobody</span>
              </>
            )}
          </div>
        ))}
      </div>
    </AnchoredPopover>
  );
}

export function CellEditor({ column, value, initialDraft, people, anchorRef, onCommit, onCancel }: Props) {
  switch (column.kind) {
    case 'select':
      return <SelectEditor column={column} value={value} onCommit={onCommit} onCancel={onCancel} />;
    case 'multi_select':
      return (
        <MultiSelectEditor
          column={column}
          value={value}
          onCommit={onCommit}
          onCancel={onCancel}
          anchorRef={anchorRef}
        />
      );
    case 'person':
      return (
        <PersonEditor
          column={column}
          value={value}
          onCommit={onCommit}
          onCancel={onCancel}
          anchorRef={anchorRef}
          people={people}
          initialDraft={initialDraft}
        />
      );
    case 'checkbox':
      return null;
    default:
      return (
        <TextEditor
          column={column}
          value={value}
          initialDraft={initialDraft}
          onCommit={onCommit}
          onCancel={onCancel}
        />
      );
  }
}
