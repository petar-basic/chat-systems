import { Select } from '@/shared/components/Form/Select';
import { TextInput } from '@/shared/components/Form/TextInput';
import { MAX_DELAY_DAYS, blankWait, timezones, type DelayStep, type DelayUnit } from './workflowModel';

const UNITS: { unit: DelayUnit; label: string }[] = [
  { unit: 'minutes', label: 'Minutes' },
  { unit: 'hours', label: 'Hours' },
  { unit: 'days', label: 'Days' },
];

interface Props {
  step: DelayStep;
  readOnly: boolean;
  onChange: (step: DelayStep) => void;
}

export function DelayEditor({ step, readOnly, onChange }: Props) {
  const wait = step.wait;
  return (
    <div className="space-y-3">
      <Select
        label="Wait"
        value={wait.kind}
        disabled={readOnly}
        onChange={(e) => onChange({ ...step, wait: blankWait(e.target.value as typeof wait.kind) })}
        dataQa="workflow-delay-kind"
      >
        <option value="for">For a while</option>
        <option value="until">Until a date and time</option>
      </Select>
      {wait.kind === 'for' ? (
        <div className="grid gap-4 sm:grid-cols-2">
          <TextInput
            label="How long"
            type="number"
            min={1}
            hint={`At most ${MAX_DELAY_DAYS} days.`}
            value={wait.amount || ''}
            disabled={readOnly}
            onChange={(e) =>
              onChange({
                ...step,
                wait: { ...wait, amount: Math.max(0, Math.floor(Number(e.target.value) || 0)) },
              })
            }
            dataQa="workflow-delay-amount"
          />
          <Select
            label="Unit"
            value={wait.unit}
            disabled={readOnly}
            onChange={(e) => onChange({ ...step, wait: { ...wait, unit: e.target.value as DelayUnit } })}
            dataQa="workflow-delay-unit"
          >
            {UNITS.map(({ unit, label }) => (
              <option key={unit} value={unit}>
                {label}
              </option>
            ))}
          </Select>
        </div>
      ) : (
        <div className="grid gap-4 sm:grid-cols-3">
          <TextInput
            label="Date"
            type="date"
            value={wait.date}
            disabled={readOnly}
            onChange={(e) => onChange({ ...step, wait: { ...wait, date: e.target.value } })}
            dataQa="workflow-delay-date"
          />
          <TextInput
            label="Time"
            type="time"
            value={wait.time}
            disabled={readOnly}
            onChange={(e) => onChange({ ...step, wait: { ...wait, time: e.target.value } })}
            dataQa="workflow-delay-time"
          />
          <Select
            label="Timezone"
            value={wait.timezone}
            disabled={readOnly}
            onChange={(e) => onChange({ ...step, wait: { ...wait, timezone: e.target.value } })}
            dataQa="workflow-delay-timezone"
          >
            {timezones(wait.timezone).map((zone) => (
              <option key={zone} value={zone}>
                {zone}
              </option>
            ))}
          </Select>
        </div>
      )}
    </div>
  );
}
