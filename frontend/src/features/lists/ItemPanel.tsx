import { useState } from 'react';
import { EyeOff, Table2, Trash2 } from 'lucide-react';
import { PanelHeader } from '@/shared/components/PanelHeader/PanelHeader';
import { TextInput } from '@/shared/components/Form/TextInput';
import { Select } from '@/shared/components/Form/Select';
import { Checkbox } from '@/shared/components/Form/Checkbox';
import { Button } from '@/shared/components/Button/Button';
import { ConfirmDialog } from '@/shared/components/ConfirmDialog/ConfirmDialog';
import { toast } from '@/shared/components/Toast';
import { displayNameOf } from '@/lib/userHelpers';
import { CellContent } from './CellContent';
import { choiceOf, draftOf, parseDraft, type ListColumn, type ListItem } from './listModel';
import type { People } from './people';

interface Props {
  item: ListItem;
  columns: ListColumn[];
  canEdit: boolean;
  people: People;
  onEdit: (columnId: string, value: unknown) => void;
  onDelete: () => void;
  onClose: () => void;
}

interface FieldProps {
  column: ListColumn;
  value: unknown;
  people: People;
  onEdit: (value: unknown) => void;
}

function DraftField({ column, value, onEdit }: Omit<FieldProps, 'people'>) {
  const [draft, setDraft] = useState(draftOf(column, value));
  const [error, setError] = useState<string | null>(null);
  const save = () => {
    const parsed = parseDraft(column.kind, draft);
    if ('error' in parsed) {
      setError(parsed.error);
      return;
    }
    if (JSON.stringify(parsed.value ?? null) !== JSON.stringify(value ?? null)) onEdit(parsed.value);
  };
  return (
    <TextInput
      label={column.name}
      labelHidden
      type={column.kind === 'date' ? 'date' : column.kind === 'link' ? 'url' : 'text'}
      inputMode={column.kind === 'number' ? 'decimal' : undefined}
      value={draft}
      error={error}
      dataQa="item-field-input"
      onChange={(e) => {
        setDraft(e.target.value);
        setError(null);
      }}
      onBlur={save}
      onKeyDown={(e) => {
        if (e.key === 'Enter') {
          e.preventDefault();
          save();
        }
      }}
    />
  );
}

function FieldEditor({ column, value, people, onEdit }: FieldProps) {
  switch (column.kind) {
    case 'checkbox':
      return (
        <Checkbox
          label={column.name}
          checked={value === true}
          onChange={onEdit}
          dataQa="item-field-checkbox"
        />
      );
    case 'select':
      return (
        <Select
          label={column.name}
          labelHidden
          value={choiceOf(column, value)?.id ?? ''}
          onChange={(e) => onEdit(e.target.value || null)}
          dataQa="item-field-select"
        >
          <option value="">—</option>
          {column.options.choices?.map((choice) => (
            <option key={choice.id} value={choice.id}>
              {choice.label}
            </option>
          ))}
        </Select>
      );
    case 'person':
      return (
        <Select
          label={column.name}
          labelHidden
          value={typeof value === 'string' ? value : ''}
          onChange={(e) => onEdit(e.target.value || null)}
          dataQa="item-field-person"
        >
          <option value="">Nobody</option>
          {typeof value === 'string' && !people.eligible.some((p) => p.id === value) && (
            <option value={value}>{people.nameOf(value)}</option>
          )}
          {people.eligible.map((person) => (
            <option key={person.id} value={person.id}>
              {person.name}
            </option>
          ))}
        </Select>
      );
    case 'multi_select': {
      const chosen = Array.isArray(value) ? value : [];
      return (
        <div className="space-y-1.5" role="group" aria-label={column.name}>
          {column.options.choices?.map((choice) => (
            <Checkbox
              key={choice.id}
              label={choice.label}
              checked={chosen.includes(choice.id)}
              onChange={(checked) => {
                const next = checked ? [...chosen, choice.id] : chosen.filter((id) => id !== choice.id);
                onEdit(next.length > 0 ? next : null);
              }}
              dataQa="item-field-choice"
            />
          ))}
        </div>
      );
    }
    default:
      return <DraftField key={JSON.stringify(value ?? null)} column={column} value={value} onEdit={onEdit} />;
  }
}

export function ItemPanel({ item, columns, canEdit, people, onEdit, onDelete, onClose }: Props) {
  const [confirming, setConfirming] = useState(false);
  const titleColumn = columns.find((column) => column.kind === 'text');
  const title = (titleColumn && (item.values[titleColumn.id] as string | undefined)) || 'Item';

  return (
    <aside
      aria-label="Item"
      data-qa="list-item-panel"
      className="w-full lg:w-96 max-lg:fixed max-lg:inset-0 max-lg:z-40 bg-surface border-l border-line/50 flex flex-col"
    >
      <PanelHeader
        title={title}
        icon={<Table2 className="h-4 w-4 text-muted" />}
        onClose={onClose}
        closeLabel="Close item"
      />
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {columns.map((column) => (
          <div key={column.id} data-qa="item-field" data-column={column.name}>
            <div className="mb-1.5 flex items-center gap-1.5 text-xs font-medium text-muted">
              {column.name}
              {column.hidden && <EyeOff className="h-3 w-3" aria-label="Hidden in the table" />}
            </div>
            {canEdit ? (
              <FieldEditor
                column={column}
                value={item.values[column.id]}
                people={people}
                onEdit={(value) => onEdit(column.id, value)}
              />
            ) : (
              <div className="min-h-6 text-sm text-fg-soft">
                <CellContent column={column} value={item.values[column.id]} people={people} />
              </div>
            )}
          </div>
        ))}
        <p className="pt-2 text-xs text-subtle">
          Added by {displayNameOf(people.nameOf(item.created_by))} on{' '}
          {new Date(item.created_at).toLocaleDateString()}
        </p>
      </div>
      {canEdit && (
        <div className="border-t border-line/50 p-3">
          <Button
            variant="ghost"
            size="sm"
            icon={<Trash2 className="h-4 w-4" />}
            onClick={() => setConfirming(true)}
            data-qa="list-item-delete"
          >
            Delete item
          </Button>
        </div>
      )}
      {confirming && (
        <ConfirmDialog
          title="Delete this item?"
          description={`"${title}" and every field in it will be removed for everyone.`}
          confirmLabel="Delete item"
          onConfirm={() => {
            onDelete();
            toast.info('Item deleted');
          }}
          onClose={() => setConfirming(false)}
          dataQa="list-item-delete-confirm"
        />
      )}
    </aside>
  );
}
