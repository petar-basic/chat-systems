import { useState, type FormEvent } from 'react';
import { Plus, X } from 'lucide-react';
import { Modal } from '@/shared/components/Modal/Modal';
import { Button } from '@/shared/components/Button/Button';
import { IconButton } from '@/shared/components/IconButton/IconButton';
import { TextInput } from '@/shared/components/Form/TextInput';
import { Select } from '@/shared/components/Form/Select';
import { toUserMessage } from '@/lib/errors';
import {
  CHOICE_COLORS,
  KIND_LABELS,
  hasChoices,
  type ChoiceColor,
  type ColumnKind,
  type ListColumn,
} from './listModel';
import type { useColumnActions } from './useLists';

interface Props {
  column: ListColumn | null;
  actions: ReturnType<typeof useColumnActions>;
  onClose: () => void;
}

interface ChoiceRow {
  key: string;
  id: string | null;
  label: string;
  color: ChoiceColor;
}

const NAME_MAX = 100;
const KINDS = Object.keys(KIND_LABELS) as ColumnKind[];

const rowsOf = (column: ListColumn | null): ChoiceRow[] =>
  (column?.options.choices ?? []).map((choice) => ({
    key: choice.id,
    id: choice.id,
    label: choice.label,
    color: choice.color,
  }));

export function ColumnDialog({ column, actions, onClose }: Props) {
  const [name, setName] = useState(column?.name ?? '');
  const [kind, setKind] = useState<ColumnKind>(column?.kind ?? 'text');
  const [rows, setRows] = useState<ChoiceRow[]>(() => rowsOf(column));
  const [error, setError] = useState<string | null>(null);
  const [clearing, setClearing] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);

  const options = {
    choices: hasChoices(kind)
      ? rows
          .filter((row) => row.label.trim())
          .map((row) => ({ id: row.id, label: row.label.trim(), color: row.color }))
      : [],
    number: column?.kind === 'number' && kind === 'number' ? column.options.number : undefined,
  };
  const reshaped =
    !!column &&
    (kind !== column.kind ||
      JSON.stringify(options.choices) !==
        JSON.stringify(rowsOf(column).map((row) => ({ id: row.id, label: row.label, color: row.color }))));

  const change =
    <T,>(set: (value: T) => void) =>
    (value: T) => {
      set(value);
      setClearing(null);
      setError(null);
    };

  const save = async () => {
    const trimmed = name.trim();
    if (!trimmed) {
      setError('A column needs a name');
      return;
    }
    setBusy(true);
    setError(null);
    try {
      if (!column) {
        await actions.add.mutateAsync({ name: trimmed, kind, options });
        onClose();
        return;
      }
      if (reshaped && clearing === null) {
        const counts = await actions.preview(column.id, { kind, options });
        if (counts.cleared > 0) {
          setClearing(counts.cleared);
          return;
        }
      }
      await actions.update.mutateAsync({
        columnId: column.id,
        body: {
          name: trimmed,
          ...(reshaped ? { kind, options } : {}),
          confirm_cleared: clearing,
        },
      });
      onClose();
    } catch (err) {
      setClearing(null);
      setError(toUserMessage(err, "Couldn't save the column"));
    } finally {
      setBusy(false);
    }
  };

  const handleSubmit = (e: FormEvent) => {
    e.preventDefault();
    void save();
  };

  const cells = clearing === 1 ? 'cell' : 'cells';

  return (
    <Modal
      title={column ? 'Edit column' : 'Add column'}
      onClose={onClose}
      onSubmit={handleSubmit}
      dataQa="list-column-dialog"
      dismissLabel="Cancel"
      actions={
        <Button
          type="submit"
          variant={clearing ? 'danger' : 'primary'}
          loading={busy}
          data-qa="list-column-save"
        >
          {clearing ? `Clear ${clearing} ${cells} and save` : column ? 'Save' : 'Add column'}
        </Button>
      }
    >
      <div className="space-y-5">
        <TextInput
          label="Name"
          value={name}
          onChange={(e) => change(setName)(e.target.value)}
          maxLength={NAME_MAX}
          autoComplete="off"
          error={error}
          dataQa="list-column-name"
          data-autofocus
        />
        <Select
          label="Kind"
          value={kind}
          onChange={(e) => change(setKind)(e.target.value as ColumnKind)}
          dataQa="list-column-kind"
        >
          {KINDS.map((k) => (
            <option key={k} value={k}>
              {KIND_LABELS[k]}
            </option>
          ))}
        </Select>
        {hasChoices(kind) && (
          <fieldset>
            <legend className="mb-1.5 text-sm font-medium text-fg-dim">Choices</legend>
            <ul className="space-y-2">
              {rows.map((row, index) => (
                <li key={row.key} className="flex items-center gap-2">
                  <TextInput
                    label={`Choice ${index + 1}`}
                    labelHidden
                    compact
                    value={row.label}
                    maxLength={NAME_MAX}
                    onChange={(e) =>
                      change(setRows)(
                        rows.map((r) => (r.key === row.key ? { ...r, label: e.target.value } : r)),
                      )
                    }
                    dataQa="list-column-choice"
                    className="flex-1"
                  />
                  <Select
                    label={`Choice ${index + 1} colour`}
                    labelHidden
                    compact
                    value={row.color}
                    onChange={(e) =>
                      change(setRows)(
                        rows.map((r) =>
                          r.key === row.key ? { ...r, color: e.target.value as ChoiceColor } : r,
                        ),
                      )
                    }
                    className="w-28"
                  >
                    {CHOICE_COLORS.map((color) => (
                      <option key={color} value={color}>
                        {color}
                      </option>
                    ))}
                  </Select>
                  <IconButton
                    label={`Remove choice ${index + 1}`}
                    icon={<X className="h-4 w-4" />}
                    tone="danger"
                    onClick={() => change(setRows)(rows.filter((r) => r.key !== row.key))}
                  />
                </li>
              ))}
            </ul>
            <Button
              size="sm"
              variant="ghost"
              className="mt-2"
              icon={<Plus className="h-4 w-4" />}
              onClick={() =>
                change(setRows)([...rows, { key: crypto.randomUUID(), id: null, label: '', color: 'gray' }])
              }
              data-qa="list-column-add-choice"
            >
              Add choice
            </Button>
          </fieldset>
        )}
        {clearing !== null && (
          <p role="alert" className="text-sm text-warning" data-qa="list-column-clearing">
            This change clears {clearing} {cells} that can't be converted. Save again to confirm.
          </p>
        )}
      </div>
    </Modal>
  );
}
