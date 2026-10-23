import { Checkbox } from '@/shared/components/Form/Checkbox';
import { Select } from '@/shared/components/Form/Select';
import { toast } from '@/shared/components/Toast';
import {
  useNotificationSchedule,
  useSetNotificationSchedule,
  type ScheduleChange,
} from '@/hooks/queries/useNotificationSchedule';
import { DAY_LABELS, EVERY_DAY, WEEKDAYS, minuteLabel, timeOptions } from './schedule';

const CHIP =
  'h-8 min-w-11 px-2 rounded-full border text-xs font-medium transition cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-purple-500 disabled:cursor-not-allowed';

export function ScheduleEditor() {
  const { data: schedule } = useNotificationSchedule();
  const save = useSetNotificationSchedule();
  if (!schedule) return null;

  const update = (patch: Partial<ScheduleChange>) =>
    save.mutate(
      {
        enabled: schedule.enabled,
        days: schedule.days,
        start_minute: schedule.start_minute,
        end_minute: schedule.end_minute,
        ...patch,
      },
      { onError: () => toast.error("Couldn't save your schedule. Try again.") },
    );

  const toggleDay = (day: number) =>
    update({
      days: schedule.days.includes(day)
        ? schedule.days.filter((d) => d !== day)
        : [...schedule.days, day].sort((a, b) => a - b),
    });

  const sameDays = (days: number[]) =>
    days.length === schedule.days.length && days.every((d) => schedule.days.includes(d));

  return (
    <div data-qa="notification-schedule" className="space-y-3">
      <Checkbox
        label="Notification schedule"
        description="Outside these hours notifications wait in Activity instead of reaching you."
        checked={schedule.enabled}
        onChange={(enabled) => update({ enabled })}
        dataQa="schedule-enabled"
      />
      {schedule.enabled && (
        <>
          <div role="group" aria-label="Days" className="flex flex-wrap items-center gap-1.5">
            {DAY_LABELS.map(({ day, short, long }) => {
              const on = schedule.days.includes(day);
              return (
                <button
                  key={day}
                  type="button"
                  aria-pressed={on}
                  aria-label={long}
                  disabled={on && schedule.days.length === 1}
                  onClick={() => toggleDay(day)}
                  data-qa={`schedule-day-${day}`}
                  className={`${CHIP} ${on ? 'border-purple-500 bg-purple-600/15 text-fg' : 'border-line text-muted hover:text-fg'}`}
                >
                  {short}
                </button>
              );
            })}
            <span className="mx-1 h-5 w-px bg-line" aria-hidden="true" />
            <button
              type="button"
              aria-pressed={sameDays(WEEKDAYS)}
              onClick={() => update({ days: WEEKDAYS })}
              data-qa="schedule-weekdays"
              className={`${CHIP} border-line text-fg-dim hover:text-fg aria-pressed:border-purple-500`}
            >
              Weekdays
            </button>
            <button
              type="button"
              aria-pressed={sameDays(EVERY_DAY)}
              onClick={() => update({ days: EVERY_DAY })}
              data-qa="schedule-every-day"
              className={`${CHIP} border-line text-fg-dim hover:text-fg aria-pressed:border-purple-500`}
            >
              Every day
            </button>
          </div>
          <div className="grid grid-cols-2 gap-3">
            <Select
              label="From"
              value={schedule.start_minute}
              onChange={(e) => {
                const start = Number(e.target.value);
                update({
                  start_minute: start,
                  end_minute: start === schedule.end_minute ? (start + 30) % 1440 : schedule.end_minute,
                });
              }}
              dataQa="schedule-start"
            >
              {timeOptions(schedule.start_minute).map((minute) => (
                <option key={minute} value={minute}>
                  {minuteLabel(minute)}
                </option>
              ))}
            </Select>
            <Select
              label="To"
              value={schedule.end_minute}
              onChange={(e) => update({ end_minute: Number(e.target.value) })}
              dataQa="schedule-end"
            >
              {timeOptions(schedule.end_minute)
                .filter((minute) => minute !== schedule.start_minute)
                .map((minute) => (
                  <option key={minute} value={minute}>
                    {minuteLabel(minute)}
                    {minute < schedule.start_minute ? ' (next day)' : ''}
                  </option>
                ))}
            </Select>
          </div>
          <p className="text-xs text-muted" data-qa="schedule-timezone">
            Times are in {schedule.timezone}, the timezone in your profile.
          </p>
        </>
      )}
    </div>
  );
}
