import type { components } from '@/api/schema';

export type Workflow = components['schemas']['Workflow'];
export type WorkflowRun = components['schemas']['WorkflowRun'];
export type Definition = components['schemas']['Definition'];
export type Trigger = components['schemas']['Trigger'];
export type Step = components['schemas']['Step'];
export type Weekday = components['schemas']['Weekday'];
export type SaveWorkflow = components['schemas']['SaveWorkflowRequest'];
export type WorkflowBuilders = components['schemas']['WorkflowBuilders'];

export const WEEKDAYS: { day: Weekday; label: string }[] = [
  { day: 'mon', label: 'Mon' },
  { day: 'tue', label: 'Tue' },
  { day: 'wed', label: 'Wed' },
  { day: 'thu', label: 'Thu' },
  { day: 'fri', label: 'Fri' },
  { day: 'sat', label: 'Sat' },
  { day: 'sun', label: 'Sun' },
];

const WORK_WEEK: Weekday[] = ['mon', 'tue', 'wed', 'thu', 'fri'];
const EVERY_DAY: Weekday[] = WEEKDAYS.map((w) => w.day);

export type Repeat = 'daily' | 'weekdays' | 'custom';

export const MAX_STEPS = 20;
export const MAX_WEBHOOK_VARIABLES = 10;
export const NAME_MAX = 80;
export const TEXT_MAX = 4000;

export function repeatOf(days: Weekday[]): Repeat {
  const set = new Set(days);
  if (set.size === 7) return 'daily';
  if (set.size === 5 && WORK_WEEK.every((d) => set.has(d))) return 'weekdays';
  return 'custom';
}

export function daysFor(repeat: Repeat, current: Weekday[]): Weekday[] {
  if (repeat === 'daily') return EVERY_DAY;
  if (repeat === 'weekdays') return WORK_WEEK;
  return current;
}

export function toggleDay(days: Weekday[], day: Weekday): Weekday[] {
  return days.includes(day)
    ? days.filter((d) => d !== day)
    : EVERY_DAY.filter((d) => d === day || days.includes(d));
}

export function localTimezone(): string {
  return Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC';
}

export function timezones(current: string): string[] {
  const known = typeof Intl.supportedValuesOf === 'function' ? Intl.supportedValuesOf('timeZone') : [];
  return known.includes(current) ? known : [current, ...known];
}

export function blankTrigger(type: Trigger['type']): Trigger {
  return type === 'schedule'
    ? { type: 'schedule', days: WORK_WEEK, time: '09:00', timezone: localTimezone() }
    : { type: 'webhook', variables: [] };
}

export function blankStep(type: Step['type'], channelId = ''): Step {
  return type === 'send_message'
    ? { type: 'send_message', channel_id: channelId, text: '' }
    : { type: 'send_dm', to: { kind: 'person', user_id: '' }, text: '' };
}

export function blankDefinition(channelId = ''): Definition {
  return { trigger: blankTrigger('schedule'), steps: [blankStep('send_message', channelId)] };
}

export function variablesOf(trigger: Trigger): { token: string; label: string }[] {
  if (trigger.type === 'webhook') {
    return trigger.variables
      .filter((name) => name.trim())
      .map((name) => ({ token: `{{webhook.${name}}}`, label: name }));
  }
  return [];
}

export function moveStep(steps: Step[], from: number, to: number): Step[] {
  if (to < 0 || to >= steps.length || from === to) return steps;
  const next = [...steps];
  const [moved] = next.splice(from, 1);
  next.splice(to, 0, moved);
  return next;
}

export function insertAt(
  text: string,
  token: string,
  start: number,
  end = start,
): { text: string; caret: number } {
  const at = Math.max(0, Math.min(start, text.length));
  const until = Math.max(at, Math.min(end, text.length));
  return { text: text.slice(0, at) + token + text.slice(until), caret: at + token.length };
}

export function variableNameError(name: string): string | null {
  if (!name) return null;
  if (!/^[a-z][a-z0-9_]*$/.test(name)) return 'Lowercase letters, digits and _, starting with a letter';
  if (name.length > 40) return 'At most 40 characters';
  return null;
}

export function triggerSummary(trigger: Trigger): string {
  if (trigger.type === 'webhook') return 'Webhook';
  const repeat = repeatOf(trigger.days);
  const when =
    repeat === 'daily'
      ? 'Every day'
      : repeat === 'weekdays'
        ? 'Weekdays'
        : WEEKDAYS.filter((w) => trigger.days.includes(w.day))
            .map((w) => w.label)
            .join(', ');
  return `${when} at ${trigger.time}`;
}

export function stepLabel(step: Step): string {
  return step.type === 'send_message' ? 'Send a message' : 'Send a DM';
}

export function workflowBuildersOf(settings: unknown): WorkflowBuilders {
  if (typeof settings !== 'object' || settings === null) return 'members';
  return (settings as Record<string, unknown>).workflow_builders === 'admins' ? 'admins' : 'members';
}

export function canBuild(role: string | null, builders: WorkflowBuilders): boolean {
  if (!role || role === 'guest') return false;
  return builders === 'members' || role === 'admin' || role === 'owner';
}

export function canManageAll(role: string | null): boolean {
  return role === 'admin' || role === 'owner';
}
