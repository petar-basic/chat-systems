import { Check } from 'lucide-react';
import { Avatar } from '@/shared/components/Avatar/Avatar';
import { CHOICE_STYLES, cellText, choiceOf, type Choice, type ListColumn } from './listModel';
import type { People } from './people';

interface Props {
  column: ListColumn;
  value: unknown;
  people: People;
}

export function ChoiceChip({ choice }: { choice: Choice }) {
  return (
    <span
      className={`inline-flex max-w-full items-center truncate rounded px-1.5 py-0.5 text-xs ${CHOICE_STYLES[choice.color]}`}
    >
      {choice.label}
    </span>
  );
}

export function CheckMark({ checked }: { checked: boolean }) {
  return (
    <span
      role="img"
      aria-label={checked ? 'Checked' : 'Not checked'}
      className={`flex h-4 w-4 items-center justify-center rounded border ${
        checked ? 'border-purple-600 bg-purple-600 text-white' : 'border-line-strong bg-raised/50'
      }`}
    >
      {checked && <Check className="h-3 w-3" strokeWidth={3} />}
    </span>
  );
}

export function CellContent({ column, value, people }: Props) {
  if (column.kind === 'checkbox') return <CheckMark checked={value === true} />;
  if (value === undefined || value === null) return null;
  switch (column.kind) {
    case 'select': {
      const choice = choiceOf(column, value);
      return choice ? <ChoiceChip choice={choice} /> : null;
    }
    case 'multi_select': {
      const chosen = Array.isArray(value) ? value.map((id) => choiceOf(column, id)).filter((c) => !!c) : [];
      return (
        <span className="flex min-w-0 gap-1 overflow-hidden">
          {chosen.map((choice) => (
            <ChoiceChip key={choice.id} choice={choice} />
          ))}
        </span>
      );
    }
    case 'person': {
      if (typeof value !== 'string') return null;
      const person = people.get(value);
      return (
        <span className="flex min-w-0 items-center gap-1.5">
          <Avatar userId={value} name={person.name} avatarUrl={person.avatarUrl} size="xs" />
          <span className="truncate">{person.name}</span>
        </span>
      );
    }
    case 'link':
      return typeof value === 'string' ? (
        <a
          href={value}
          target="_blank"
          rel="noreferrer noopener"
          tabIndex={-1}
          className="truncate text-accent-soft hover:underline"
          onClick={(e) => e.stopPropagation()}
        >
          {value.replace(/^https?:\/\//, '')}
        </a>
      ) : null;
    case 'number':
      return (
        <span className="block w-full truncate text-right tabular-nums">
          {cellText(column, value, people.nameOf)}
        </span>
      );
    default:
      return <span className="truncate">{cellText(column, value, people.nameOf)}</span>;
  }
}
