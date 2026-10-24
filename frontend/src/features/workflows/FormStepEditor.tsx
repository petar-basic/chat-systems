import { Plus, Trash2 } from 'lucide-react';
import { Button } from '@/shared/components/Button/Button';
import { Checkbox } from '@/shared/components/Form/Checkbox';
import { Select } from '@/shared/components/Form/Select';
import { TextArea } from '@/shared/components/Form/TextArea';
import { TextInput } from '@/shared/components/Form/TextInput';
import { IconButton } from '@/shared/components/IconButton/IconButton';
import {
  FIELD_KINDS,
  FIELD_LABEL_MAX,
  FORM_TITLE_MAX,
  MAX_FORM_FIELDS,
  blankField,
  relabel,
  type FieldKind,
  type FormField,
  type FormStep,
} from './workflowModel';

interface Props {
  step: FormStep;
  readOnly: boolean;
  onChange: (step: FormStep) => void;
}

export function FormStepEditor({ step, readOnly, onChange }: Props) {
  const setFields = (fields: FormField[]) => onChange({ ...step, fields });
  const setField = (index: number, patch: Partial<FormField>) =>
    setFields(step.fields.map((field, i) => (i === index ? { ...field, ...patch } : field)));

  return (
    <div className="space-y-3">
      <TextInput
        label="Form title"
        value={step.title}
        maxLength={FORM_TITLE_MAX}
        disabled={readOnly}
        placeholder="Request time off"
        onChange={(e) => onChange({ ...step, title: e.target.value })}
        dataQa="workflow-form-title"
      />
      <ol className="space-y-3" aria-label="Questions">
        {step.fields.map((field, index) => (
          <li
            key={index}
            className="space-y-3 rounded-lg border border-line p-3"
            data-qa="workflow-form-field"
          >
            <div className="grid items-end gap-3 sm:grid-cols-[1fr_10rem_auto]">
              <TextInput
                label={`Question ${index + 1}`}
                value={field.label}
                maxLength={FIELD_LABEL_MAX}
                disabled={readOnly}
                placeholder="How many days?"
                onChange={(e) => setFields(relabel(step.fields, index, e.target.value))}
                dataQa="workflow-form-field-label"
              />
              <Select
                label="Answer"
                value={field.kind}
                disabled={readOnly}
                onChange={(e) => {
                  const kind = e.target.value as FieldKind;
                  setField(index, {
                    kind,
                    options: kind === 'select' && !field.options?.length ? [''] : field.options,
                  });
                }}
                dataQa="workflow-form-field-kind"
              >
                {FIELD_KINDS.map(({ kind, label }) => (
                  <option key={kind} value={kind}>
                    {label}
                  </option>
                ))}
              </Select>
              {!readOnly && (
                <IconButton
                  label={`Remove question ${index + 1}`}
                  icon={<Trash2 className="h-4 w-4" />}
                  tone="danger"
                  disabled={step.fields.length === 1}
                  onClick={() => setFields(step.fields.filter((_, i) => i !== index))}
                  data-qa="workflow-form-field-remove"
                />
              )}
            </div>
            {field.kind === 'select' && (
              <TextArea
                label="Choices"
                hint="One per line"
                rows={3}
                value={(field.options ?? []).join('\n')}
                disabled={readOnly}
                onChange={(e) => setField(index, { options: e.target.value.split('\n') })}
                dataQa="workflow-form-field-options"
              />
            )}
            <div className="flex flex-wrap items-center gap-x-4 gap-y-1">
              <Checkbox
                label="Required"
                checked={field.required ?? false}
                disabled={readOnly}
                onChange={(required) => setField(index, { required })}
                dataQa="workflow-form-field-required"
              />
              <span className="text-xs text-muted" data-qa="workflow-form-field-variable">
                Later steps use it as <code>{`{{form.${field.name}}}`}</code>
              </span>
            </div>
          </li>
        ))}
      </ol>
      {!readOnly && step.fields.length < MAX_FORM_FIELDS && (
        <Button
          size="sm"
          variant="ghost"
          icon={<Plus className="h-4 w-4" />}
          onClick={() => setFields([...step.fields, blankField(step.fields)])}
          data-qa="workflow-form-field-add"
        >
          Add a question
        </Button>
      )}
    </div>
  );
}
