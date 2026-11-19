import { X } from 'lucide-react';
import { useQueries } from '@tanstack/react-query';
import { Checkbox } from '@/shared/components/Form/Checkbox';
import { Select } from '@/shared/components/Form/Select';
import { TextInput } from '@/shared/components/Form/TextInput';
import { IconButton } from '@/shared/components/IconButton/IconButton';
import { displayNameOf } from '@/lib/userHelpers';
import type { WorkspaceMember } from '@/stores/workspace';
import { listDetailQuery, type ListColumn } from '@/features/lists';
import { useProfileFields, type ProfileField } from '@/features/people';
import {
  MAX_TEST_VALUES,
  TEST_VALUE_MAX,
  branchVariables,
  emptyTest,
  listStepsBefore,
  opLabel,
  opsFor,
  testFor,
  triggerPerson,
  variableKind,
  withOp,
  type BranchStep,
  type Definition,
  type Test,
  type TestOp,
  type VariableKind,
} from './workflowModel';

interface Props {
  step: BranchStep;
  position: number;
  definition: Definition;
  workspaceId: string;
  instanceUrl?: string;
  members: WorkspaceMember[];
  readOnly: boolean;
  onChange: (step: BranchStep) => void;
}

interface Subject {
  value: string;
  label: string;
  kind: VariableKind;
}

function profileKind(field: ProfileField): VariableKind {
  if (field.kind === 'person') return { kind: 'person' };
  if (field.kind === 'date') return { kind: 'date' };
  if (field.kind === 'options') return { kind: 'choice', options: field.options.map((o) => o.label) };
  return { kind: 'text' };
}

function columnKind(column: ListColumn): VariableKind {
  if (column.kind === 'person') return { kind: 'person' };
  if (column.kind === 'date') return { kind: 'date' };
  if (column.kind === 'select')
    return { kind: 'choice', options: (column.options.choices ?? []).map((c) => c.label) };
  return { kind: 'text' };
}

export function BranchEditor({
  step,
  position,
  definition,
  workspaceId,
  instanceUrl,
  members,
  readOnly,
  onChange,
}: Props) {
  const person = triggerPerson(definition.trigger);
  const { fields } = useProfileFields(person ? workspaceId : undefined, instanceUrl);
  const listSteps = listStepsBefore(definition.steps, position);
  const lists = useQueries({
    queries: listSteps.map((at) => listDetailQuery(at.listId, instanceUrl)),
  });

  const variables: Subject[] = branchVariables(definition, position).map((variable) => ({
    value: variable.token,
    label: variable.label,
    kind: variableKind(definition, position, variable.token),
  }));
  const profile: Subject[] = person
    ? fields.map((field) => ({
        value: `profile.${field.id}`,
        label: `Their profile: ${field.label}`,
        kind: profileKind(field),
      }))
    : [];
  const columns: Subject[] = listSteps.flatMap((at, i) =>
    (lists[i]?.data?.columns ?? []).map((column) => ({
      value: `step.${at.position + 1}.column.${column.id}`,
      label: `Step ${at.position + 1} item: ${column.name}`,
      kind: columnKind(column),
    })),
  );
  const subjects = [...variables, ...profile, ...columns];
  const subject = subjects.find((s) => s.value === step.value);
  const kind = subject?.kind ?? { kind: 'text' };

  const pick = (value: string) => {
    const next = subjects.find((s) => s.value === value);
    if (!next) return;
    const test =
      next.kind.kind === kind.kind ? testFor(step.test, next.kind) : emptyTest(opsFor(next.kind)[0]);
    onChange({ ...step, value, test });
  };

  const groups = [
    { label: 'From the trigger and forms', subjects: variables },
    { label: 'Their profile', subjects: profile },
    { label: 'List items', subjects: columns },
  ].filter((group) => group.subjects.length > 0);

  return (
    <div className="space-y-3">
      <div className="grid gap-4 sm:grid-cols-2">
        <Select
          label="It checks"
          value={step.value}
          disabled={readOnly}
          onChange={(e) => pick(e.target.value)}
          dataQa="workflow-branch-value"
        >
          <option value="" disabled>
            {subjects.length ? 'Pick a value' : 'Nothing to check yet'}
          </option>
          {step.value && !subject && <option value={step.value}>{step.value}</option>}
          {groups.map((group) => (
            <optgroup key={group.label} label={group.label}>
              {group.subjects.map((s) => (
                <option key={s.value} value={s.value}>
                  {s.label}
                </option>
              ))}
            </optgroup>
          ))}
        </Select>
        <Select
          label="Condition"
          value={step.test.op}
          disabled={readOnly}
          onChange={(e) => onChange({ ...step, test: withOp(step.test, e.target.value as TestOp) })}
          dataQa="workflow-branch-op"
        >
          {opsFor(kind).map((op) => (
            <option key={op} value={op}>
              {opLabel(op, kind)}
            </option>
          ))}
        </Select>
      </div>
      <Operands
        test={step.test}
        kind={kind}
        members={members}
        readOnly={readOnly}
        onChange={(test) => onChange({ ...step, test })}
      />
      <p className="text-xs text-muted">
        Each path runs its own steps, then the workflow carries on with the step after the branch.
      </p>
    </div>
  );
}

function Operands({
  test,
  kind,
  members,
  readOnly,
  onChange,
}: {
  test: Test;
  kind: VariableKind;
  members: WorkspaceMember[];
  readOnly: boolean;
  onChange: (test: Test) => void;
}) {
  if (test.op === 'is_empty') return null;
  const nameOf = (id: string) =>
    displayNameOf(members.find((m) => m.user_id === id)?.display_name ?? 'Someone who left');

  if (test.op === 'one_of') {
    const values = test.values;
    const setValues = (next: string[]) => onChange({ ...test, values: next.slice(0, MAX_TEST_VALUES) });
    if (kind.kind === 'choice') {
      return (
        <fieldset className="space-y-2" data-qa="workflow-branch-values">
          <legend className="mb-1.5 text-sm font-medium text-fg-dim">Any of</legend>
          {kind.options.map((option) => (
            <Checkbox
              key={option}
              label={option}
              checked={values.includes(option)}
              disabled={readOnly}
              onChange={(on) => setValues(on ? [...values, option] : values.filter((v) => v !== option))}
            />
          ))}
        </fieldset>
      );
    }
    if (kind.kind === 'person') {
      const rest = members.filter((m) => !values.includes(m.user_id));
      return (
        <div className="space-y-2" data-qa="workflow-branch-values">
          <ul className="flex flex-wrap gap-2" aria-label="People it matches">
            {values.map((id) => (
              <li
                key={id}
                className="flex items-center gap-1 rounded-full bg-raised py-0.5 pl-3 pr-1 text-sm"
              >
                {nameOf(id)}
                {!readOnly && (
                  <IconButton
                    label={`Remove ${nameOf(id)}`}
                    icon={<X className="h-3.5 w-3.5" />}
                    onClick={() => setValues(values.filter((v) => v !== id))}
                  />
                )}
              </li>
            ))}
          </ul>
          {!readOnly && values.length < MAX_TEST_VALUES && (
            <Select
              label="Add a person"
              value=""
              onChange={(e) => e.target.value && setValues([...values, e.target.value])}
              dataQa="workflow-branch-person-add"
            >
              <option value="">Pick a person</option>
              {rest.map((m) => (
                <option key={m.user_id} value={m.user_id}>
                  {displayNameOf(m.display_name)}
                </option>
              ))}
            </Select>
          )}
        </div>
      );
    }
    return (
      <TextInput
        label="Any of"
        hint="Separate them with commas."
        value={values.join(',')}
        disabled={readOnly}
        placeholder={kind.kind === 'date' ? '2026-12-24, 2026-12-31' : 'bug, outage'}
        onChange={(e) => setValues(e.target.value.split(','))}
        dataQa="workflow-branch-values"
      />
    );
  }

  const setValue = (value: string) => onChange({ ...test, value });
  if (test.op === 'equals' && kind.kind === 'person') {
    return (
      <Select
        label="Person"
        value={test.value}
        disabled={readOnly}
        onChange={(e) => setValue(e.target.value)}
        dataQa="workflow-branch-operand"
      >
        <option value="" disabled>
          Pick a person
        </option>
        {members.map((m) => (
          <option key={m.user_id} value={m.user_id}>
            {displayNameOf(m.display_name)}
          </option>
        ))}
      </Select>
    );
  }
  if (test.op === 'equals' && kind.kind === 'choice') {
    return (
      <Select
        label="Choice"
        value={kind.options.includes(test.value) ? test.value : ''}
        disabled={readOnly}
        onChange={(e) => setValue(e.target.value)}
        dataQa="workflow-branch-operand"
      >
        <option value="" disabled>
          Pick a choice
        </option>
        {kind.options.map((option) => (
          <option key={option} value={option}>
            {option}
          </option>
        ))}
      </Select>
    );
  }
  return (
    <TextInput
      label={kind.kind === 'date' ? 'Date' : test.op === 'greater_than' ? 'Number or date' : 'Value'}
      type={kind.kind === 'date' ? 'date' : 'text'}
      value={test.value}
      maxLength={TEST_VALUE_MAX}
      disabled={readOnly}
      placeholder={test.op === 'greater_than' ? '500' : 'urgent'}
      onChange={(e) => setValue(e.target.value)}
      dataQa="workflow-branch-operand"
    />
  );
}
