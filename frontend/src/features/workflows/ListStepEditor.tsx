import { Wand2 } from 'lucide-react';
import { Button } from '@/shared/components/Button/Button';
import { Select } from '@/shared/components/Form/Select';
import { TextInput } from '@/shared/components/Form/TextInput';
import { Spinner } from '@/shared/components/Spinner/Spinner';
import { displayNameOf } from '@/lib/userHelpers';
import type { WorkspaceMember } from '@/stores/workspace';
import { KIND_LABELS, type ListColumn, useListDetail, useWorkspaceLists } from '@/features/lists';
import {
  MAX_CELLS,
  TEXT_MAX,
  UPDATABLE_KINDS,
  addStepsBefore,
  cellFor,
  fits,
  formsBefore,
  listOf,
  matchColumns,
  setCell,
  variableKind,
  type Definition,
  type ListStep,
  type Variable,
} from './workflowModel';

interface Props {
  step: ListStep;
  index: number;
  definition: Definition;
  workspaceId: string;
  instanceUrl?: string;
  members: WorkspaceMember[];
  variables: Variable[];
  readOnly: boolean;
  onChange: (step: ListStep) => void;
}

export function ListStepEditor({
  step,
  index,
  definition,
  workspaceId,
  instanceUrl,
  members,
  variables,
  readOnly,
  onChange,
}: Props) {
  const lists = useWorkspaceLists(workspaceId, instanceUrl, step.type === 'add_list_item');
  const listId = listOf(definition.steps, index);
  const earlier = addStepsBefore(definition.steps, index);

  return (
    <div className="space-y-3">
      {step.type === 'add_list_item' ? (
        <Select
          label="List"
          value={step.list_id}
          disabled={readOnly}
          onChange={(e) => onChange({ ...step, list_id: e.target.value, cells: [] })}
          dataQa="workflow-step-list"
        >
          <option value="" disabled>
            {lists.isLoading ? 'Loading lists…' : 'Pick a list'}
          </option>
          {(lists.data ?? []).map((list) => (
            <option key={list.id} value={list.id}>
              {list.title}
            </option>
          ))}
        </Select>
      ) : (
        <Select
          label="The item added by"
          value={earlier.includes(step.from_step) ? String(step.from_step) : ''}
          disabled={readOnly}
          onChange={(e) => onChange({ ...step, from_step: Number(e.target.value), cells: [] })}
          dataQa="workflow-step-from"
        >
          <option value="" disabled>
            {earlier.length ? 'Pick a step' : 'Add a list item in an earlier step first'}
          </option>
          {earlier.map((at) => (
            <option key={at} value={at}>
              Step {at + 1} · Add a list item
            </option>
          ))}
        </Select>
      )}
      {listId && (
        <CellFields
          key={listId}
          listId={listId}
          step={step}
          index={index}
          definition={definition}
          instanceUrl={instanceUrl}
          members={members}
          variables={variables}
          readOnly={readOnly}
          onChange={onChange}
        />
      )}
    </div>
  );
}

function CellFields({
  listId,
  step,
  index,
  definition,
  instanceUrl,
  members,
  variables,
  readOnly,
  onChange,
}: Omit<Props, 'workspaceId'> & { listId: string }) {
  const detail = useListDetail(listId, instanceUrl);

  if (detail.isLoading) {
    return (
      <div className="flex justify-center py-3 text-accent">
        <Spinner />
      </div>
    );
  }
  if (!detail.data) {
    return (
      <p className="text-sm text-danger" data-qa="workflow-step-list-missing">
        You can no longer open that list.
      </p>
    );
  }

  const updating = step.type === 'update_list_item';
  const columns = detail.data.columns.filter((c) => !updating || UPDATABLE_KINDS.includes(c.kind));
  const matched = !updating && formsBefore(definition.steps, index).length > 0;
  const full = step.cells.length >= MAX_CELLS;
  const fitting = (column: ListColumn) =>
    variables.filter((v) => fits(column, variableKind(definition, index, v.token)));
  const setValue = (column: ListColumn, value: string) =>
    onChange({ ...step, cells: setCell(step.cells, column.id, value) });

  if (columns.length === 0) {
    return (
      <p className="text-sm text-muted" data-qa="workflow-step-no-columns">
        {updating ? 'That list has no person, choice or checkbox column.' : 'That list has no columns yet.'}
      </p>
    );
  }

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between gap-3">
        <p className="text-xs text-muted">
          {updating
            ? 'Set these on the item. Leave one empty to keep it as is.'
            : 'Leave a column empty to skip it.'}
        </p>
        {matched && !readOnly && (
          <Button
            size="sm"
            variant="ghost"
            icon={<Wand2 className="h-3.5 w-3.5" />}
            onClick={() => onChange({ ...step, cells: matchColumns(columns, definition, index) })}
            data-qa="workflow-step-match"
          >
            Fill from the form answers
          </Button>
        )}
      </div>
      <ul className="space-y-3" aria-label="Columns">
        {columns.map((column) => {
          const value = cellFor(step.cells, column.id);
          return (
            <li key={column.id} data-qa="workflow-cell" data-column={column.name}>
              <CellField
                column={column}
                value={value}
                variables={fitting(column)}
                members={members}
                disabled={readOnly || (full && !value)}
                onChange={(next) => setValue(column, next)}
              />
            </li>
          );
        })}
      </ul>
    </div>
  );
}

function CellField({
  column,
  value,
  variables,
  members,
  disabled,
  onChange,
}: {
  column: ListColumn;
  value: string;
  variables: Variable[];
  members: WorkspaceMember[];
  disabled: boolean;
  onChange: (value: string) => void;
}) {
  const label = `${column.name} · ${KIND_LABELS[column.kind]}`;

  if (column.kind === 'select' || column.kind === 'person' || column.kind === 'checkbox') {
    const fixed =
      column.kind === 'select'
        ? (column.options.choices ?? []).map((c) => ({ value: c.id, label: c.label }))
        : column.kind === 'person'
          ? members.map((m) => ({ value: m.user_id, label: displayNameOf(m.display_name) }))
          : [
              { value: 'true', label: 'Checked' },
              { value: 'false', label: 'Unchecked' },
            ];
    const known = fixed.some((o) => o.value === value) || variables.some((v) => v.token === value);
    return (
      <Select
        label={label}
        value={value}
        disabled={disabled}
        onChange={(e) => onChange(e.target.value)}
        dataQa="workflow-cell-value"
      >
        <option value="">Leave empty</option>
        {value && !known && <option value={value}>{value}</option>}
        {fixed.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
        {variables.length > 0 && (
          <optgroup label="Variables">
            {variables.map((variable) => (
              <option key={variable.token} value={variable.token}>
                {variable.label}
              </option>
            ))}
          </optgroup>
        )}
      </Select>
    );
  }

  const free = column.kind === 'text' || column.kind === 'link';
  return (
    <div className="grid items-end gap-2 sm:grid-cols-[1fr_12rem]">
      <TextInput
        label={label}
        value={value}
        maxLength={TEXT_MAX}
        disabled={disabled}
        placeholder={column.kind === 'date' ? 'YYYY-MM-DD' : column.kind === 'multi_select' ? 'One, Two' : ''}
        onChange={(e) => onChange(e.target.value)}
        dataQa="workflow-cell-value"
      />
      {variables.length > 0 && !disabled && (
        <Select
          label={`Insert a variable into ${column.name}`}
          labelHidden
          value=""
          onChange={(e) => e.target.value && onChange(free ? value + e.target.value : e.target.value)}
          dataQa="workflow-cell-variable"
        >
          <option value="">Insert variable…</option>
          {variables.map((variable) => (
            <option key={variable.token} value={variable.token}>
              {variable.label}
            </option>
          ))}
        </Select>
      )}
    </div>
  );
}
