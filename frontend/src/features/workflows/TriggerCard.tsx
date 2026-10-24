import { Clock, Plus, Trash2, Webhook } from 'lucide-react';
import { Select } from '@/shared/components/Form/Select';
import { TextInput } from '@/shared/components/Form/TextInput';
import { Checkbox } from '@/shared/components/Form/Checkbox';
import { Button } from '@/shared/components/Button/Button';
import { IconButton } from '@/shared/components/IconButton/IconButton';
import { CopyField } from '@/shared/components/CopyField/CopyField';
import {
  MAX_WEBHOOK_VARIABLES,
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
  webhookUrl: string | null;
  readOnly: boolean;
  onChange: (trigger: Trigger) => void;
}

export function TriggerCard({ trigger, webhookUrl, readOnly, onChange }: Props) {
  return (
    <section className="rounded-xl border border-line bg-raised/30 p-4" data-qa="workflow-trigger">
      <h3 className="mb-3 flex items-center gap-2 text-sm font-semibold text-fg">
        {trigger.type === 'schedule' ? <Clock className="h-4 w-4" /> : <Webhook className="h-4 w-4" />}
        Starts
      </h3>
      <div className="space-y-4">
        <Select
          label="Trigger"
          value={trigger.type}
          disabled={readOnly}
          onChange={(e) => onChange(blankTrigger(e.target.value as Trigger['type']))}
          dataQa="workflow-trigger-type"
        >
          <option value="schedule">On a schedule</option>
          <option value="webhook">When a webhook is called</option>
        </Select>
        {trigger.type === 'schedule' ? (
          <ScheduleFields trigger={trigger} readOnly={readOnly} onChange={onChange} />
        ) : (
          <WebhookFields trigger={trigger} webhookUrl={webhookUrl} readOnly={readOnly} onChange={onChange} />
        )}
      </div>
    </section>
  );
}

type Schedule = Extract<Trigger, { type: 'schedule' }>;
type Hook = Extract<Trigger, { type: 'webhook' }>;

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
