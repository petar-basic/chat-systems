import { AlertTriangle } from 'lucide-react';
import { Select } from '@/shared/components/Form/Select';
import { TextArea } from '@/shared/components/Form/TextArea';
import { TextInput } from '@/shared/components/Form/TextInput';
import { displayNameOf } from '@/lib/userHelpers';
import type { Channel, WorkspaceMember } from '@/stores/workspace';
import { VariablePicker } from './VariablePicker';
import { useVariableInsert } from './useVariableInsert';
import {
  TEXT_MAX,
  appStepProblem,
  loneToken,
  offeredStep,
  stepVariables,
  typedVariables,
  type AppStep,
  type Definition,
  type Step,
  type StepParam,
  type StepProvider,
  type Variable,
} from './workflowModel';

const ENTERED = '';

interface Props {
  step: AppStep;
  position: number;
  definition: Definition;
  channels: Channel[];
  members: WorkspaceMember[];
  providers?: StepProvider[];
  readOnly: boolean;
  onChange: (step: Step) => void;
}

export function AppStepEditor({
  step,
  position,
  definition,
  channels,
  members,
  providers,
  readOnly,
  onChange,
}: Props) {
  const { provider, offered } = offeredStep(providers, step);
  const problem = appStepProblem(step, providers);
  const inputs = step.inputs ?? [];
  const outputs = step.outputs ?? [];
  const values = step.values ?? {};
  const set = (name: string, value: string) => onChange({ ...step, values: { ...values, [name]: value } });

  return (
    <div className="space-y-4" data-qa="workflow-app-step">
      {problem && (
        <p
          role="alert"
          className="flex items-start gap-2 rounded-lg bg-warning/10 px-3 py-2 text-sm text-warning"
          data-qa="workflow-app-warning"
        >
          <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" aria-hidden />
          {problem}
        </p>
      )}
      {(provider || offered?.description) && (
        <p className="text-xs text-muted">
          {provider && <span className="font-medium text-fg-soft">{provider.name}</span>}
          {provider && offered?.description && ' · '}
          {offered?.description}
        </p>
      )}
      {inputs.length === 0 ? (
        <p className="text-xs text-muted">This step takes no inputs.</p>
      ) : (
        inputs.map((input) => (
          <InputField
            key={input.name}
            input={input}
            value={values[input.name] ?? ''}
            definition={definition}
            position={position}
            channels={channels}
            members={members}
            readOnly={readOnly}
            onChange={(value) => set(input.name, value)}
          />
        ))
      )}
      {outputs.length > 0 && (
        <p className="text-xs text-muted" data-qa="workflow-app-outputs">
          Later steps can use what it hands back: {outputs.map((output) => output.title).join(', ')}.
        </p>
      )}
    </div>
  );
}

interface FieldProps {
  input: StepParam;
  value: string;
  definition: Definition;
  position: number;
  channels: Channel[];
  members: WorkspaceMember[];
  readOnly: boolean;
  onChange: (value: string) => void;
}

function InputField(props: FieldProps) {
  const { input, definition, position } = props;
  if (input.kind === 'text') {
    return <TextField {...props} variables={stepVariables(definition, position)} />;
  }
  return <TypedField {...props} variables={typedVariables(definition, position, input.kind)} />;
}

function TextField({ input, value, variables, readOnly, onChange }: FieldProps & { variables: Variable[] }) {
  const { ref, insert } = useVariableInsert<HTMLTextAreaElement>(value, onChange);
  return (
    <div className="space-y-2">
      <TextArea
        ref={ref}
        label={input.title}
        optional={!input.required}
        rows={2}
        value={value}
        maxLength={TEXT_MAX}
        disabled={readOnly}
        onChange={(e) => onChange(e.target.value)}
        dataQa={`workflow-app-input-${input.name}`}
      />
      {!readOnly && (
        <VariablePicker
          variables={variables}
          label={`Insert a variable into ${input.title}`}
          onPick={insert}
          dataQa={`workflow-app-variable-${input.name}`}
        />
      )}
    </div>
  );
}

function TypedField({
  input,
  value,
  variables,
  channels,
  members,
  readOnly,
  onChange,
}: FieldProps & { variables: Variable[] }) {
  const token = loneToken(value);
  const dataQa = `workflow-app-input-${input.name}`;

  if (input.kind === 'person' || input.kind === 'channel' || input.kind === 'boolean') {
    const fixed =
      input.kind === 'person'
        ? members.map((member) => ({ value: member.user_id, label: displayNameOf(member.display_name) }))
        : input.kind === 'channel'
          ? channels.map((channel) => ({ value: channel.id, label: `#${channel.name}` }))
          : [
              { value: 'true', label: 'Yes' },
              { value: 'false', label: 'No' },
            ];
    const placeholder =
      input.kind === 'person' ? 'Pick a person' : input.kind === 'channel' ? 'Pick a channel' : 'Pick one';
    return (
      <Select
        label={input.title}
        optional={!input.required}
        value={token ?? value}
        disabled={readOnly}
        onChange={(e) => onChange(e.target.value)}
        dataQa={dataQa}
      >
        <option value="">{input.required ? placeholder : 'Leave it empty'}</option>
        {variables.length > 0 && (
          <optgroup label="From this workflow">
            {variables.map((variable) => (
              <option key={variable.token} value={variable.token}>
                {variable.label}
              </option>
            ))}
          </optgroup>
        )}
        <optgroup
          label={input.kind === 'person' ? 'People' : input.kind === 'channel' ? 'Channels' : 'Value'}
        >
          {fixed.map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
            </option>
          ))}
        </optgroup>
      </Select>
    );
  }

  return (
    <div className="grid items-end gap-2 sm:grid-cols-2">
      {variables.length > 0 && (
        <Select
          label={input.title}
          optional={!input.required}
          value={token ?? ENTERED}
          disabled={readOnly}
          onChange={(e) => onChange(e.target.value)}
          dataQa={`${dataQa}-source`}
        >
          <option value={ENTERED}>{input.kind === 'date' ? 'Pick a date' : 'Type a number'}</option>
          {variables.map((variable) => (
            <option key={variable.token} value={variable.token}>
              {variable.label}
            </option>
          ))}
        </Select>
      )}
      {!token && (
        <TextInput
          label={variables.length > 0 ? `${input.title}: value` : input.title}
          labelHidden={variables.length > 0}
          optional={variables.length === 0 && !input.required}
          type={input.kind === 'date' ? 'date' : 'text'}
          inputMode={input.kind === 'number' ? 'decimal' : undefined}
          value={value}
          disabled={readOnly}
          placeholder={input.kind === 'number' ? '3' : undefined}
          onChange={(e) => onChange(e.target.value)}
          dataQa={dataQa}
        />
      )}
    </div>
  );
}
