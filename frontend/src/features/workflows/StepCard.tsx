import { ArrowDown, ArrowUp, Trash2 } from 'lucide-react';
import { Checkbox } from '@/shared/components/Form/Checkbox';
import { Select } from '@/shared/components/Form/Select';
import { TextArea } from '@/shared/components/Form/TextArea';
import { IconButton } from '@/shared/components/IconButton/IconButton';
import { displayNameOf } from '@/lib/userHelpers';
import type { Channel, WorkspaceMember } from '@/stores/workspace';
import { BranchEditor } from './BranchEditor';
import {
  ChannelRefSelect,
  CreateChannelFields,
  EmojiField,
  InviteeFields,
  TopicFields,
} from './ChannelStepEditor';
import { DelayEditor } from './DelayEditor';
import { FormStepEditor } from './FormStepEditor';
import { ListStepEditor } from './ListStepEditor';
import { VariablePicker } from './VariablePicker';
import { useVariableInsert } from './useVariableInsert';
import {
  MAX_BRANCH_DEPTH,
  TEXT_MAX,
  addStepsBefore,
  blankStep,
  channelRefFor,
  formsBefore,
  stepLabel,
  stepVariables,
  triggerPerson as personOf,
  type Definition,
  type FlatStep,
  type Step,
  type TextStep,
  type Variable,
} from './workflowModel';

const TRIGGER_PERSON = 'trigger_user';

const ACTIONS: { type: Step['type']; label: string }[] = [
  { type: 'send_message', label: 'Send a message to a channel' },
  { type: 'send_dm', label: 'Send a DM to a person' },
  { type: 'form', label: 'Ask a person to fill in a form' },
  { type: 'add_list_item', label: 'Add an item to a list' },
  { type: 'update_list_item', label: 'Update the item a step added' },
  { type: 'branch', label: 'Branch on a condition' },
  { type: 'delay', label: 'Wait' },
  { type: 'add_reaction', label: 'React to the message' },
  { type: 'reply_in_thread', label: 'Reply in the message’s thread' },
  { type: 'create_channel', label: 'Create a channel' },
  { type: 'add_to_channel', label: 'Add people to a channel' },
  { type: 'set_topic', label: 'Set a channel’s topic' },
  { type: 'archive_channel', label: 'Archive a channel' },
];

interface Props {
  node: FlatStep;
  definition: Definition;
  workspaceId: string;
  instanceUrl?: string;
  channels: Channel[];
  people: WorkspaceMember[];
  members: WorkspaceMember[];
  removable: boolean;
  readOnly: boolean;
  onChange: (step: Step) => void;
  onMove: (offset: number) => void;
  onRemove: () => void;
}

export function StepCard({
  node,
  definition,
  workspaceId,
  instanceUrl,
  channels,
  people,
  members,
  removable,
  readOnly,
  onChange,
  onMove,
  onRemove,
}: Props) {
  const { step, position } = node;
  const number = position + 1;
  const earlier = addStepsBefore(definition.steps, position);
  const variables = stepVariables(definition, position);
  const triggerPerson = personOf(definition.trigger);
  const onMessage = definition.trigger.type === 'reaction_added';

  const offered = (type: Step['type']) => {
    if (type === step.type) return true;
    if (type === 'update_list_item') return earlier.length > 0;
    if (type === 'branch') return node.depth < MAX_BRANCH_DEPTH;
    if (type === 'add_reaction' || type === 'reply_in_thread') return onMessage;
    return true;
  };

  const changeType = (type: Step['type']) => {
    const next = blankStep(
      type,
      '',
      earlier[earlier.length - 1] ?? position,
      channelRefFor(definition, position),
    );
    if ('text' in next && 'text' in step) {
      onChange({ ...next, text: step.text });
    } else if ('to' in next && 'to' in step) {
      onChange({ ...next, to: step.to });
    } else if ('channel' in next && 'channel' in step) {
      onChange({ ...next, channel: step.channel });
    } else {
      onChange(next);
    }
  };

  const channelRef =
    step.type === 'add_to_channel' || step.type === 'set_topic' || step.type === 'archive_channel' ? (
      <ChannelRefSelect
        channel={step.channel}
        position={position}
        definition={definition}
        channels={channels}
        readOnly={readOnly}
        onChange={(channel) => onChange({ ...step, channel })}
      />
    ) : null;

  return (
    <section
      className="rounded-xl border border-line bg-surface p-4"
      aria-label={`Step ${number}`}
      data-qa="workflow-step"
      data-type={step.type}
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
              disabled={node.index === 0}
              onClick={() => onMove(-1)}
              data-qa="workflow-step-up"
            />
            <IconButton
              label={`Move step ${number} down`}
              icon={<ArrowDown className="h-4 w-4" />}
              disabled={node.index === node.siblings - 1}
              onClick={() => onMove(1)}
              data-qa="workflow-step-down"
            />
            <IconButton
              label={`Remove step ${number}`}
              icon={<Trash2 className="h-4 w-4" />}
              tone="danger"
              disabled={!removable}
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
            {ACTIONS.filter((action) => offered(action.type)).map((action) => (
              <option key={action.type} value={action.type}>
                {action.label}
              </option>
            ))}
          </Select>
          {step.type === 'send_message' ? (
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
          ) : step.type === 'send_dm' || step.type === 'form' ? (
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
          ) : (
            channelRef
          )}
        </div>
        {step.type === 'form' ? (
          <FormStepEditor step={step} readOnly={readOnly} onChange={onChange} />
        ) : step.type === 'add_list_item' || step.type === 'update_list_item' ? (
          <ListStepEditor
            step={step}
            index={position}
            definition={definition}
            workspaceId={workspaceId}
            instanceUrl={instanceUrl}
            members={members}
            variables={variables}
            readOnly={readOnly}
            onChange={onChange}
          />
        ) : step.type === 'branch' ? (
          <BranchEditor
            step={step}
            position={position}
            definition={definition}
            workspaceId={workspaceId}
            instanceUrl={instanceUrl}
            members={members}
            readOnly={readOnly}
            onChange={onChange}
          />
        ) : step.type === 'delay' ? (
          <DelayEditor step={step} readOnly={readOnly} onChange={onChange} />
        ) : step.type === 'add_reaction' ? (
          <EmojiField step={step} readOnly={readOnly} onChange={onChange} />
        ) : step.type === 'create_channel' ? (
          <CreateChannelFields step={step} variables={variables} readOnly={readOnly} onChange={onChange} />
        ) : step.type === 'add_to_channel' ? (
          <InviteeFields
            step={step}
            position={position}
            definition={definition}
            workspaceId={workspaceId}
            instanceUrl={instanceUrl}
            triggerPerson={triggerPerson}
            readOnly={readOnly}
            onChange={onChange}
          />
        ) : step.type === 'set_topic' ? (
          <TopicFields step={step} variables={variables} readOnly={readOnly} onChange={onChange} />
        ) : step.type === 'archive_channel' ? (
          <p className="text-xs text-muted">
            It archives the channel when the run gets here, as you. You have to be allowed to archive it.
          </p>
        ) : (
          <MessageFields
            step={step}
            variables={variables}
            formBefore={formsBefore(definition.steps, position).length > 0}
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
  const { ref, insert } = useVariableInsert<HTMLTextAreaElement>(step.text, (text) =>
    onChange({ ...step, text }),
  );
  const summary = step.type === 'reply_in_thread' ? null : step;

  return (
    <>
      <TextArea
        ref={ref}
        label={step.type === 'reply_in_thread' ? 'Reply' : 'Message'}
        value={step.text}
        rows={4}
        maxLength={TEXT_MAX}
        disabled={readOnly}
        placeholder={
          step.type === 'send_message'
            ? 'Standup in 10 minutes!'
            : step.type === 'reply_in_thread'
              ? 'On it — thanks for flagging.'
              : 'Welcome aboard!'
        }
        onChange={(e) => onChange({ ...step, text: e.target.value })}
        dataQa="workflow-step-text"
      />
      <div className="flex flex-wrap items-center gap-4">
        {!readOnly && <VariablePicker variables={variables} onPick={insert} />}
        {summary && (formBefore || summary.summary) && (
          <Checkbox
            label="Add the form answers below the message"
            checked={summary.summary ?? false}
            disabled={readOnly}
            onChange={(on) => onChange({ ...summary, summary: on })}
            dataQa="workflow-step-summary"
          />
        )}
      </div>
    </>
  );
}
