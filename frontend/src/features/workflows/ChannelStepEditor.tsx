import { useRef, useState } from 'react';
import { X } from 'lucide-react';
import { EmojiPicker } from '@/features/messaging';
import { ReactionEmoji } from '@/shared/components/ReactionEmoji';
import { Button } from '@/shared/components/Button/Button';
import { Checkbox } from '@/shared/components/Form/Checkbox';
import { Field } from '@/shared/components/Form/Field';
import { Select } from '@/shared/components/Form/Select';
import { TextArea } from '@/shared/components/Form/TextArea';
import { TextInput } from '@/shared/components/Form/TextInput';
import { IconButton } from '@/shared/components/IconButton/IconButton';
import type { Channel } from '@/stores/workspace';
import { useUserGroups } from '@/features/workspace';
import { VariablePicker } from './VariablePicker';
import { useVariableInsert } from './useVariableInsert';
import {
  CHANNEL_NAME_MAX,
  MAX_INVITEES,
  TOPIC_MAX,
  channelStepsBefore,
  personQuestionsBefore,
  type ChannelRef,
  type Definition,
  type Invitee,
  type Step,
  type Variable,
} from './workflowModel';

type CreateStep = Extract<Step, { type: 'create_channel' }>;
type InviteStep = Extract<Step, { type: 'add_to_channel' }>;
type TopicStep = Extract<Step, { type: 'set_topic' }>;
type ReactStep = Extract<Step, { type: 'add_reaction' }>;

function refValue(ref: ChannelRef): string {
  if (ref.kind === 'trigger_channel') return 'trigger';
  if (ref.kind === 'from_step') return `step:${ref.step}`;
  return ref.channel_id ? `channel:${ref.channel_id}` : '';
}

function refFrom(value: string): ChannelRef {
  if (value === 'trigger') return { kind: 'trigger_channel' };
  if (value.startsWith('step:')) return { kind: 'from_step', step: Number(value.slice('step:'.length)) };
  return { kind: 'channel', channel_id: value.slice('channel:'.length) };
}

export function ChannelRefSelect({
  channel,
  position,
  definition,
  channels,
  readOnly,
  onChange,
}: {
  channel: ChannelRef;
  position: number;
  definition: Definition;
  channels: Channel[];
  readOnly: boolean;
  onChange: (channel: ChannelRef) => void;
}) {
  const created = channelStepsBefore(definition.steps, position);
  const triggered = 'channel_id' in definition.trigger;
  const options = [
    ...(triggered ? [{ value: 'trigger', label: 'The channel it started in' }] : []),
    ...created.map((at) => ({ value: `step:${at}`, label: `The channel step ${at + 1} creates` })),
    ...channels.map((c) => ({ value: `channel:${c.id}`, label: `#${c.name}` })),
  ];
  const value = refValue(channel);
  return (
    <Select
      label="Channel"
      value={options.some((o) => o.value === value) ? value : ''}
      disabled={readOnly}
      onChange={(e) => onChange(refFrom(e.target.value))}
      dataQa="workflow-step-channel-ref"
    >
      <option value="" disabled>
        Pick a channel
      </option>
      {options.map((option) => (
        <option key={option.value} value={option.value}>
          {option.label}
        </option>
      ))}
    </Select>
  );
}

export function CreateChannelFields({
  step,
  variables,
  readOnly,
  onChange,
}: {
  step: CreateStep;
  variables: Variable[];
  readOnly: boolean;
  onChange: (step: CreateStep) => void;
}) {
  const { ref, insert } = useVariableInsert<HTMLInputElement>(step.name, (name) =>
    onChange({ ...step, name }),
  );
  return (
    <div className="space-y-3">
      <TextInput
        ref={ref}
        label="Channel name"
        hint="It’s made lowercase, with dashes for spaces. A name that’s taken stops the run."
        value={step.name}
        maxLength={CHANNEL_NAME_MAX}
        disabled={readOnly}
        placeholder="proj-{{form.project}}"
        onChange={(e) => onChange({ ...step, name: e.target.value })}
        dataQa="workflow-step-channel-name"
      />
      <div className="flex flex-wrap items-center gap-4">
        {!readOnly && <VariablePicker variables={variables} onPick={insert} />}
        <Checkbox
          label="Make it private"
          checked={step.private ?? false}
          disabled={readOnly}
          onChange={(on) => onChange({ ...step, private: on })}
          dataQa="workflow-step-channel-private"
        />
      </div>
    </div>
  );
}

export function TopicFields({
  step,
  variables,
  readOnly,
  onChange,
}: {
  step: TopicStep;
  variables: Variable[];
  readOnly: boolean;
  onChange: (step: TopicStep) => void;
}) {
  const { ref, insert } = useVariableInsert<HTMLTextAreaElement>(step.topic, (topic) =>
    onChange({ ...step, topic }),
  );
  return (
    <div className="space-y-3">
      <TextArea
        ref={ref}
        label="Topic"
        rows={2}
        value={step.topic}
        maxLength={TOPIC_MAX}
        disabled={readOnly}
        placeholder="Launch on {{form.date}}"
        onChange={(e) => onChange({ ...step, topic: e.target.value })}
        dataQa="workflow-step-topic"
      />
      {!readOnly && <VariablePicker variables={variables} onPick={insert} />}
    </div>
  );
}

function inviteeKey(invitee: Invitee): string {
  if (invitee.kind === 'trigger_user') return 'trigger';
  if (invitee.kind === 'answer') return `answer:${invitee.field}`;
  return `group:${invitee.group_id}`;
}

function inviteeFrom(key: string): Invitee {
  if (key === 'trigger') return { kind: 'trigger_user' };
  if (key.startsWith('answer:')) return { kind: 'answer', field: key.slice('answer:'.length) };
  return { kind: 'group', group_id: key.slice('group:'.length) };
}

export function InviteeFields({
  step,
  position,
  definition,
  workspaceId,
  instanceUrl,
  triggerPerson,
  readOnly,
  onChange,
}: {
  step: InviteStep;
  position: number;
  definition: Definition;
  workspaceId: string;
  instanceUrl?: string;
  triggerPerson: string | null;
  readOnly: boolean;
  onChange: (step: InviteStep) => void;
}) {
  const groups = useUserGroups(workspaceId, instanceUrl);
  const options = [
    ...(triggerPerson ? [{ key: 'trigger', label: triggerPerson }] : []),
    ...personQuestionsBefore(definition.steps, position).map((field) => ({
      key: `answer:${field.name}`,
      label: `Answer: ${field.label || field.name}`,
    })),
    ...(groups.data ?? []).map((group) => ({ key: `group:${group.id}`, label: `@${group.handle}` })),
  ];
  const chosen = step.people.map(inviteeKey);
  const labelOf = (key: string) =>
    options.find((o) => o.key === key)?.label ??
    (key.startsWith('group:') ? 'A deleted group' : 'A question that’s gone');
  const rest = options.filter((o) => !chosen.includes(o.key));

  return (
    <div className="space-y-2">
      <p className="text-sm font-medium text-fg-dim">People to add</p>
      {chosen.length > 0 ? (
        <ul className="flex flex-wrap gap-2" aria-label="People to add" data-qa="workflow-step-invitees">
          {chosen.map((key) => (
            <li key={key} className="flex items-center gap-1 rounded-full bg-raised py-0.5 pl-3 pr-1 text-sm">
              {labelOf(key)}
              {!readOnly && (
                <IconButton
                  label={`Don’t add ${labelOf(key)}`}
                  icon={<X className="h-3.5 w-3.5" />}
                  onClick={() =>
                    onChange({ ...step, people: step.people.filter((p) => inviteeKey(p) !== key) })
                  }
                />
              )}
            </li>
          ))}
        </ul>
      ) : (
        <p className="text-xs text-muted">Nobody yet.</p>
      )}
      {!readOnly && chosen.length < MAX_INVITEES && (
        <Select
          label="Add people"
          labelHidden
          className="w-64"
          value=""
          onChange={(e) =>
            e.target.value && onChange({ ...step, people: [...step.people, inviteeFrom(e.target.value)] })
          }
          dataQa="workflow-step-invitee-add"
        >
          <option value="">{rest.length ? 'Add people…' : 'Nobody else to add'}</option>
          {rest.map((option) => (
            <option key={option.key} value={option.key}>
              {option.label}
            </option>
          ))}
        </Select>
      )}
    </div>
  );
}

export function EmojiField({
  step,
  readOnly,
  onChange,
}: {
  step: ReactStep;
  readOnly: boolean;
  onChange: (step: ReactStep) => void;
}) {
  const anchor = useRef<HTMLSpanElement>(null);
  const [picking, setPicking] = useState(false);
  return (
    <Field label="Emoji" hint="It reacts to the message that set it off, as you.">
      {(control) => (
        <span ref={anchor} className="inline-block">
          <Button
            {...control}
            disabled={readOnly}
            onClick={() => setPicking((open) => !open)}
            data-qa="workflow-step-emoji"
          >
            {step.emoji ? (
              <span className="text-lg leading-none">
                <ReactionEmoji emoji={step.emoji} />
              </span>
            ) : (
              'Pick an emoji'
            )}
          </Button>
          {picking && (
            <EmojiPicker
              anchorRef={anchor}
              onSelect={(emoji) => onChange({ ...step, emoji })}
              onClose={() => setPicking(false)}
            />
          )}
        </span>
      )}
    </Field>
  );
}
