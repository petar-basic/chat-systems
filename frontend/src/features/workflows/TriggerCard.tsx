import { useRef, useState } from 'react';
import { Clock, Plus, SmilePlus, Trash2, UserPlus, Webhook, Zap } from 'lucide-react';
import { EmojiPicker } from '@/features/messaging';
import { ReactionEmoji } from '@/shared/components/ReactionEmoji';
import type { Channel } from '@/stores/workspace';
import { Field } from '@/shared/components/Form/Field';
import { Select } from '@/shared/components/Form/Select';
import { TextInput } from '@/shared/components/Form/TextInput';
import { Checkbox } from '@/shared/components/Form/Checkbox';
import { Button } from '@/shared/components/Button/Button';
import { IconButton } from '@/shared/components/IconButton/IconButton';
import { CopyField } from '@/shared/components/CopyField/CopyField';
import {
  MAX_WEBHOOK_VARIABLES,
  TRIGGER_TYPES,
  WEEKDAYS,
  blankTrigger,
  daysFor,
  repeatOf,
  timezones,
  toggleDay,
  variableNameError,
  type Repeat,
  type Trigger,
} from './workflowModel';

interface Props {
  trigger: Trigger;
  channels: Channel[];
  webhookUrl: string | null;
  readOnly: boolean;
  onChange: (trigger: Trigger) => void;
}

const ICONS: Record<Trigger['type'], typeof Clock> = {
  schedule: Clock,
  webhook: Webhook,
  reaction_added: SmilePlus,
  member_joined: UserPlus,
  shortcut: Zap,
};

export function TriggerCard({ trigger, channels, webhookUrl, readOnly, onChange }: Props) {
  const Icon = ICONS[trigger.type];
  return (
    <section className="rounded-xl border border-line bg-raised/30 p-4" data-qa="workflow-trigger">
      <h3 className="mb-3 flex items-center gap-2 text-sm font-semibold text-fg">
        <Icon className="h-4 w-4" />
        Starts
      </h3>
      <div className="space-y-4">
        <Select
          label="Trigger"
          value={trigger.type}
          disabled={readOnly}
          onChange={(e) => onChange(blankTrigger(e.target.value as Trigger['type'], channels[0]?.id))}
          dataQa="workflow-trigger-type"
        >
          {TRIGGER_TYPES.map(({ type, label }) => (
            <option key={type} value={type}>
              {label}
            </option>
          ))}
        </Select>
        {trigger.type === 'schedule' && (
          <ScheduleFields trigger={trigger} readOnly={readOnly} onChange={onChange} />
        )}
        {trigger.type === 'webhook' && (
          <WebhookFields trigger={trigger} webhookUrl={webhookUrl} readOnly={readOnly} onChange={onChange} />
        )}
        {(trigger.type === 'reaction_added' ||
          trigger.type === 'member_joined' ||
          trigger.type === 'shortcut') && (
          <EventFields trigger={trigger} channels={channels} readOnly={readOnly} onChange={onChange} />
        )}
      </div>
    </section>
  );
}

type Schedule = Extract<Trigger, { type: 'schedule' }>;
type Hook = Extract<Trigger, { type: 'webhook' }>;
type ChannelEvent = Extract<Trigger, { type: 'reaction_added' | 'member_joined' | 'shortcut' }>;

const CHANNEL_LABEL: Record<ChannelEvent['type'], string> = {
  reaction_added: 'In',
  member_joined: 'Joins',
  shortcut: 'Shortcut in',
};

function EventFields({
  trigger,
  channels,
  readOnly,
  onChange,
}: {
  trigger: ChannelEvent;
  channels: Channel[];
  readOnly: boolean;
  onChange: (trigger: Trigger) => void;
}) {
  const anchor = useRef<HTMLSpanElement>(null);
  const [picking, setPicking] = useState(false);
  return (
    <div className="grid gap-4 sm:grid-cols-2">
      <Select
        label={CHANNEL_LABEL[trigger.type]}
        value={trigger.channel_id}
        disabled={readOnly}
        onChange={(e) => onChange({ ...trigger, channel_id: e.target.value })}
        dataQa="workflow-trigger-channel"
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
      {trigger.type === 'reaction_added' && (
        <Field label="Emoji">
          {(control) => (
            <span ref={anchor} className="inline-block">
              <Button
                {...control}
                disabled={readOnly}
                onClick={() => setPicking((open) => !open)}
                data-qa="workflow-trigger-emoji"
              >
                {trigger.emoji ? (
                  <span className="text-lg leading-none">
                    <ReactionEmoji emoji={trigger.emoji} />
                  </span>
                ) : (
                  'Pick an emoji'
                )}
              </Button>
              {picking && (
                <EmojiPicker
                  anchorRef={anchor}
                  onSelect={(emoji) => onChange({ ...trigger, emoji })}
                  onClose={() => setPicking(false)}
                />
              )}
            </span>
          )}
        </Field>
      )}
    </div>
  );
}

function ScheduleFields({
  trigger,
  readOnly,
  onChange,
}: {
  trigger: Schedule;
  readOnly: boolean;
  onChange: (trigger: Trigger) => void;
}) {
  const repeat = repeatOf(trigger.days);
  return (
    <>
      <div className="grid gap-4 sm:grid-cols-3">
        <Select
          label="Repeats"
          value={repeat}
          disabled={readOnly}
          onChange={(e) => onChange({ ...trigger, days: daysFor(e.target.value as Repeat, trigger.days) })}
          dataQa="workflow-schedule-repeat"
        >
          <option value="daily">Every day</option>
          <option value="weekdays">Weekdays</option>
          <option value="custom">Chosen days</option>
        </Select>
        <TextInput
          label="At"
          type="time"
          value={trigger.time}
          disabled={readOnly}
          onChange={(e) => onChange({ ...trigger, time: e.target.value })}
          dataQa="workflow-schedule-time"
        />
        <Select
          label="Timezone"
          value={trigger.timezone}
          disabled={readOnly}
          onChange={(e) => onChange({ ...trigger, timezone: e.target.value })}
          dataQa="workflow-schedule-timezone"
        >
          {timezones(trigger.timezone).map((zone) => (
            <option key={zone} value={zone}>
              {zone.replaceAll('_', ' ')}
            </option>
          ))}
        </Select>
      </div>
      {repeat === 'custom' && (
        <fieldset className="flex flex-wrap gap-x-4 gap-y-2" data-qa="workflow-schedule-days">
          <legend className="sr-only">Days</legend>
          {WEEKDAYS.map(({ day, label }) => (
            <Checkbox
              key={day}
              label={label}
              checked={trigger.days.includes(day)}
              disabled={readOnly}
              onChange={() => onChange({ ...trigger, days: toggleDay(trigger.days, day) })}
              dataQa={`workflow-day-${day}`}
            />
          ))}
        </fieldset>
      )}
    </>
  );
}

function WebhookFields({
  trigger,
  webhookUrl,
  readOnly,
  onChange,
}: {
  trigger: Hook;
  webhookUrl: string | null;
  readOnly: boolean;
  onChange: (trigger: Trigger) => void;
}) {
  const setVariables = (variables: string[]) => onChange({ ...trigger, variables });
  return (
    <div className="space-y-3">
      {webhookUrl ? (
        <CopyField label="Webhook URL" value={webhookUrl} valueQa="workflow-webhook-url" />
      ) : (
        <p className="text-xs text-muted">Save the workflow to get its URL.</p>
      )}
      <p className="text-xs text-muted">
        POST a JSON object to it. Each variable below is read from the key with the same name.
      </p>
      {trigger.variables.map((name, index) => (
        <div key={index} className="flex items-start gap-2" data-qa="workflow-variable">
          <TextInput
            label={`Variable ${index + 1}`}
            labelHidden
            className="flex-1"
            value={name}
            placeholder="service"
            disabled={readOnly}
            error={variableNameError(name)}
            onChange={(e) =>
              setVariables(trigger.variables.map((v, i) => (i === index ? e.target.value.trim() : v)))
            }
            dataQa="workflow-variable-name"
          />
          {!readOnly && (
            <IconButton
              label={`Remove ${name || `variable ${index + 1}`}`}
              icon={<Trash2 className="h-4 w-4" />}
              tone="danger"
              size="md"
              onClick={() => setVariables(trigger.variables.filter((_, i) => i !== index))}
              data-qa="workflow-variable-remove"
            />
          )}
        </div>
      ))}
      {!readOnly && trigger.variables.length < MAX_WEBHOOK_VARIABLES && (
        <Button
          size="sm"
          variant="ghost"
          icon={<Plus className="h-3.5 w-3.5" />}
          onClick={() => setVariables([...trigger.variables, ''])}
          data-qa="workflow-variable-add"
        >
          Add variable
        </Button>
      )}
    </div>
  );
}
