import { useRef } from 'react';
import { ArrowDown, ArrowUp, Trash2 } from 'lucide-react';
import { Checkbox } from '@/shared/components/Form/Checkbox';
import { Select } from '@/shared/components/Form/Select';
import { TextArea } from '@/shared/components/Form/TextArea';
import { IconButton } from '@/shared/components/IconButton/IconButton';
import { displayNameOf } from '@/lib/userHelpers';
import type { Channel, WorkspaceMember } from '@/stores/workspace';
import { FormStepEditor } from './FormStepEditor';
import { ListStepEditor } from './ListStepEditor';
import {
  TEXT_MAX,
  addStepsBefore,
  blankStep,
  insertAt,
  stepLabel,
  type Definition,
  type Step,
  type TextStep,
  type Variable,
} from './workflowModel';

const TRIGGER_PERSON = 'trigger_user';

interface Props {
  step: Step;
  index: number;
  count: number;
  definition: Definition;
  workspaceId: string;
  instanceUrl?: string;
  channels: Channel[];
  people: WorkspaceMember[];
  members: WorkspaceMember[];
  variables: Variable[];
  triggerPerson: string | null;
  formBefore: boolean;
  readOnly: boolean;
  onChange: (step: Step) => void;
  onMove: (to: number) => void;
  onRemove: () => void;
}

export function StepCard({
  step,
  index,
  count,
  definition,
  workspaceId,
  instanceUrl,
  channels,
  people,
  members,
  variables,
  triggerPerson,
  formBefore,
  readOnly,
  onChange,
  onMove,
  onRemove,
}: Props) {
  const number = index + 1;

  const earlier = addStepsBefore(definition.steps, index);

  const changeType = (type: Step['type']) => {
    const next = blankStep(type, '', earlier[earlier.length - 1] ?? index);
    if ((next.type === 'send_message' || next.type === 'send_dm') && 'text' in step) {
      onChange({ ...next, text: step.text });
    } else if ((next.type === 'send_dm' || next.type === 'form') && 'to' in step) {
      onChange({ ...next, to: step.to });
    } else {
      onChange(next);
    }
  };

  return (
    <section
      className="rounded-xl border border-line bg-surface p-4"
      aria-label={`Step ${number}`}
      data-qa="workflow-step"
    >
      <header className="mb-3 flex items-center gap-2">
        <span className="flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-accent/15 text-xs font-semibold text-accent">
          {number}
        </span>
        <h3 className="flex-1 truncate text-sm font-semibold text-fg" data-qa="workflow-step-title">
          {stepLabel(step)}
        </h3>
        {!readOnly && (
          <div className="flex gap-0.5">
            <IconButton
              label={`Move step ${number} up`}
              icon={<ArrowUp className="h-4 w-4" />}
              disabled={index === 0}
              onClick={() => onMove(index - 1)}
              data-qa="workflow-step-up"
            />
            <IconButton
              label={`Move step ${number} down`}
              icon={<ArrowDown className="h-4 w-4" />}
              disabled={index === count - 1}
              onClick={() => onMove(index + 1)}
              data-qa="workflow-step-down"
            />
            <IconButton
              label={`Remove step ${number}`}
              icon={<Trash2 className="h-4 w-4" />}
              tone="danger"
              disabled={count === 1}
              onClick={onRemove}
              data-qa="workflow-step-remove"
            />
          </div>
        )}
      </header>
      <div className="space-y-4">
        <div className="grid gap-4 sm:grid-cols-2">
          <Select
            label="Action"
            value={step.type}
            disabled={readOnly}
            onChange={(e) => changeType(e.target.value as Step['type'])}
            dataQa="workflow-step-type"
          >
            <option value="send_message">Send a message to a channel</option>
            <option value="send_dm">Send a DM to a person</option>
            <option value="form">Ask a person to fill in a form</option>
            <option value="add_list_item">Add an item to a list</option>
            {(earlier.length > 0 || step.type === 'update_list_item') && (
              <option value="update_list_item">Update the item a step added</option>
            )}
          </Select>
          {step.type === 'add_list_item' || step.type === 'update_list_item' ? null : step.type ===
            'send_message' ? (
            <Select
              label="Channel"
              value={step.channel_id}
              disabled={readOnly}
              onChange={(e) => onChange({ ...step, channel_id: e.target.value })}
              dataQa="workflow-step-channel"
            >
              <option value="" disabled>
                Pick a channel
              </option>
              {channels.map((channel) => (
                <option key={channel.id} value={channel.id}>
                  #{channel.name}
                </option>
              ))}
            </Select>
          ) : (
            <Select
              label={step.type === 'form' ? 'Send it to' : 'To'}
              value={step.to.kind === 'person' ? step.to.user_id : triggerPerson ? TRIGGER_PERSON : ''}
              disabled={readOnly}
              onChange={(e) =>
                onChange({
                  ...step,
                  to:
                    e.target.value === TRIGGER_PERSON
                      ? { kind: 'trigger_user' }
                      : { kind: 'person', user_id: e.target.value },
                })
              }
              dataQa="workflow-step-person"
            >
              <option value="" disabled>
                Pick a person
              </option>
              {triggerPerson && <option value={TRIGGER_PERSON}>{triggerPerson}</option>}
              {people.map((person) => (
                <option key={person.user_id} value={person.user_id}>
                  {displayNameOf(person.display_name)}
                </option>
              ))}
            </Select>
          )}
        </div>
        {step.type === 'form' ? (
          <FormStepEditor step={step} readOnly={readOnly} onChange={onChange} />
        ) : step.type === 'add_list_item' || step.type === 'update_list_item' ? (
          <ListStepEditor
            step={step}
            index={index}
            definition={definition}
            workspaceId={workspaceId}
            instanceUrl={instanceUrl}
            members={members}
            variables={variables}
            readOnly={readOnly}
            onChange={onChange}
          />
        ) : (
          <MessageFields
            step={step}
            variables={variables}
            formBefore={formBefore}
            readOnly={readOnly}
            onChange={onChange}
          />
        )}
      </div>
    </section>
  );
}

function MessageFields({
  step,
  variables,
  formBefore,
  readOnly,
  onChange,
}: {
  step: TextStep;
  variables: Variable[];
  formBefore: boolean;
  readOnly: boolean;
  onChange: (step: Step) => void;
}) {
  const textRef = useRef<HTMLTextAreaElement>(null);

  const insertVariable = (token: string) => {
    const area = textRef.current;
    const start = area?.selectionStart ?? step.text.length;
    const end = area?.selectionEnd ?? start;
    const { text, caret } = insertAt(step.text, token, start, end);
    onChange({ ...step, text });
    requestAnimationFrame(() => {
      area?.focus();
      area?.setSelectionRange(caret, caret);
    });
  };

  return (
    <>
      <TextArea
        ref={textRef}
        label="Message"
        value={step.text}
        rows={4}
        maxLength={TEXT_MAX}
        disabled={readOnly}
        placeholder={step.type === 'send_message' ? 'Standup in 10 minutes!' : 'Welcome aboard!'}
        onChange={(e) => onChange({ ...step, text: e.target.value })}
        dataQa="workflow-step-text"
      />
      <div className="flex flex-wrap items-center gap-4">
        {!readOnly && variables.length > 0 && (
          <Select
            label="Insert variable"
            labelHidden
            compact
            className="w-48"
            value=""
            onChange={(e) => e.target.value && insertVariable(e.target.value)}
            dataQa="workflow-step-variable"
          >
            <option value="">Insert variable…</option>
            {variables.map((variable) => (
              <option key={variable.token} value={variable.token}>
                {variable.label}
              </option>
            ))}
          </Select>
        )}
        {(formBefore || step.summary) && (
          <Checkbox
            label="Add the form answers below the message"
            checked={step.summary ?? false}
            disabled={readOnly}
            onChange={(summary) => onChange({ ...step, summary })}
            dataQa="workflow-step-summary"
          />
        )}
      </div>
    </>
  );
}
