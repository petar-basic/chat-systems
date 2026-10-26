import type { components } from '@/api/schema';
import type { ListColumn } from '@/features/lists';

export type Workflow = components['schemas']['Workflow'];
export type WorkflowRun = components['schemas']['WorkflowRun'];
export type Definition = components['schemas']['Definition'];
export type Trigger = components['schemas']['Trigger'];
export type Step = components['schemas']['Step'];
export type Weekday = components['schemas']['Weekday'];
export type SaveWorkflow = components['schemas']['SaveWorkflowRequest'];
export type WorkflowBuilders = components['schemas']['WhoMay'];
export type FormField = components['schemas']['FormField'];
export type FieldKind = components['schemas']['FieldKind'];
export type FormStep = Extract<Step, { type: 'form' }>;
export type TextStep = Extract<Step, { type: 'send_message' | 'send_dm' }>;
export type AddItemStep = Extract<Step, { type: 'add_list_item' }>;
export type UpdateItemStep = Extract<Step, { type: 'update_list_item' }>;
export type ListStep = AddItemStep | UpdateItemStep;
export type CellValue = components['schemas']['CellValue'];
export type Variable = { token: string; label: string };
export type FormRequest = components['schemas']['FormRequest'];
export type WorkflowShortcut = components['schemas']['Shortcut'];

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
export const FORM_TITLE_MAX = 80;
export const MAX_FORM_FIELDS = 10;
export const FIELD_LABEL_MAX = 80;
export const MAX_SELECT_OPTIONS = 20;
export const SHORT_ANSWER_MAX = 200;
export const LONG_ANSWER_MAX = 4000;
export const MAX_CELLS = 30;
export const UPDATABLE_KINDS: ListColumn['kind'][] = ['person', 'select', 'checkbox'];

export const FIELD_KINDS: { kind: FieldKind; label: string }[] = [
  { kind: 'short_text', label: 'Short answer' },
  { kind: 'long_text', label: 'Long answer' },
  { kind: 'select', label: 'Choice' },
  { kind: 'person', label: 'Person' },
  { kind: 'date', label: 'Date' },
];

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

export const TRIGGER_TYPES: { type: Trigger['type']; label: string }[] = [
  { type: 'schedule', label: 'On a schedule' },
  { type: 'webhook', label: 'When a webhook is called' },
  { type: 'reaction_added', label: 'When a reaction is added' },
  { type: 'member_joined', label: 'When someone joins a channel' },
  { type: 'shortcut', label: 'From a shortcut in a channel' },
];

export function blankTrigger(type: Trigger['type'], channelId = ''): Trigger {
  switch (type) {
    case 'schedule':
      return { type: 'schedule', days: WORK_WEEK, time: '09:00', timezone: localTimezone() };
    case 'webhook':
      return { type: 'webhook', variables: [] };
    case 'reaction_added':
      return { type: 'reaction_added', channel_id: channelId, emoji: '' };
    case 'member_joined':
      return { type: 'member_joined', channel_id: channelId };
    case 'shortcut':
      return { type: 'shortcut', channel_id: channelId };
  }
}

export function triggerPerson(trigger: Trigger): string | null {
  if (trigger.type === 'reaction_added') return 'The person who reacted';
  if (trigger.type === 'member_joined') return 'The person who joined';
  if (trigger.type === 'shortcut') return 'The person who ran it';
  return null;
}

export function blankStep(type: Step['type'], channelId = '', fromStep = 0): Step {
  switch (type) {
    case 'send_message':
      return { type: 'send_message', channel_id: channelId, text: '' };
    case 'send_dm':
      return { type: 'send_dm', to: { kind: 'person', user_id: '' }, text: '' };
    case 'form':
      return { type: 'form', to: { kind: 'person', user_id: '' }, title: '', fields: [blankField([])] };
    case 'add_list_item':
      return { type: 'add_list_item', list_id: '', cells: [] };
    case 'update_list_item':
      return { type: 'update_list_item', from_step: fromStep, cells: [] };
  }
}

export function addStepsBefore(steps: Step[], index: number): number[] {
  return steps.flatMap((step, i) => (i < index && step.type === 'add_list_item' ? [i] : []));
}

export function listOf(steps: Step[], index: number): string | null {
  const step = steps[index];
  if (step?.type === 'add_list_item') return step.list_id || null;
  if (step?.type === 'update_list_item' && step.from_step < index) return listOf(steps, step.from_step);
  return null;
}

export type VariableKind =
  { kind: 'text' } | { kind: 'person' } | { kind: 'date' } | { kind: 'choice'; options: string[] };

export function variableKind(definition: Definition, index: number, token: string): VariableKind {
  const name = token.replace(/^\{\{\s*|\s*\}\}$/g, '');
  if (name === 'trigger.user') return { kind: 'person' };
  const field = name.startsWith('form.')
    ? formsBefore(definition.steps, index)
        .flatMap((form) => form.fields)
        .find((f) => f.name === name.slice('form.'.length))
    : undefined;
  if (field?.kind === 'person') return { kind: 'person' };
  if (field?.kind === 'date') return { kind: 'date' };
  if (field?.kind === 'select') return { kind: 'choice', options: field.options ?? [] };
  return { kind: 'text' };
}

export function fits(column: ListColumn, variable: VariableKind): boolean {
  if (variable.kind === 'text') return true;
  if (variable.kind === 'person') return column.kind === 'person';
  if (variable.kind === 'date') return column.kind === 'date';
  if (column.kind !== 'select' && column.kind !== 'multi_select') return false;
  const labels = (column.options.choices ?? []).map((c) => c.label.toLowerCase());
  return variable.options.every((option) => labels.includes(option.trim().toLowerCase()));
}

export function setCell(cells: CellValue[], columnId: string, value: string): CellValue[] {
  const rest = cells.filter((cell) => cell.column_id !== columnId);
  if (!value) return rest;
  return cells.some((cell) => cell.column_id === columnId)
    ? cells.map((cell) => (cell.column_id === columnId ? { ...cell, value } : cell))
    : [...cells, { column_id: columnId, value }];
}

export function cellFor(cells: CellValue[], columnId: string): string {
  return cells.find((cell) => cell.column_id === columnId)?.value ?? '';
}

export function matchColumns(columns: ListColumn[], definition: Definition, index: number): CellValue[] {
  const answers = formsBefore(definition.steps, index).flatMap((form) => form.fields);
  return columns.flatMap((column) => {
    const field = answers.find((f) => f.label.trim().toLowerCase() === column.name.trim().toLowerCase());
    if (!field?.name) return [];
    const token = `{{form.${field.name}}}`;
    return fits(column, variableKind(definition, index, token))
      ? [{ column_id: column.id, value: token }]
      : [];
  });
}

export function fieldNameFrom(label: string, taken: string[]): string {
  const base =
    label
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '_')
      .replace(/^[^a-z]+/, '')
      .replace(/_+$/, '')
      .slice(0, 32) || 'answer';
  if (!taken.includes(base)) return base;
  let n = 2;
  while (taken.includes(`${base}_${n}`)) n += 1;
  return `${base}_${n}`;
}

export function blankField(fields: FormField[]): FormField {
  return {
    name: fieldNameFrom(
      '',
      fields.map((f) => f.name),
    ),
    label: '',
    kind: 'short_text',
    options: [],
    required: true,
  };
}

export function relabel(fields: FormField[], index: number, label: string): FormField[] {
  return fields.map((field, i) => {
    if (i !== index) return field;
    const others = fields.filter((_, j) => j !== index).map((f) => f.name);
    const named = field.name === fieldNameFrom(field.label, others);
    return { ...field, label, name: named ? fieldNameFrom(label, others) : field.name };
  });
}

export function formsBefore(steps: Step[], index: number): FormStep[] {
  return steps.slice(0, index).filter((step): step is FormStep => step.type === 'form');
}

export function stepVariables(definition: Definition, index: number): Variable[] {
  const asked = formsBefore(definition.steps, index).flatMap((form) =>
    form.fields
      .filter((field) => field.name)
      .map((field) => ({ token: `{{form.${field.name}}}`, label: `Answer: ${field.label || field.name}` })),
  );
  return [...variablesOf(definition.trigger), ...asked];
}

export function blankDefinition(channelId = ''): Definition {
  return { trigger: blankTrigger('schedule'), steps: [blankStep('send_message', channelId)] };
}

export function variablesOf(trigger: Trigger): Variable[] {
  switch (trigger.type) {
    case 'webhook':
      return trigger.variables
        .filter((name) => name.trim())
        .map((name) => ({ token: `{{webhook.${name}}}`, label: name }));
    case 'reaction_added':
      return [
        { token: '{{trigger.user}}', label: 'Person who reacted' },
        { token: '{{trigger.channel}}', label: 'Channel' },
        { token: '{{trigger.message}}', label: 'Message they reacted to' },
        { token: '{{trigger.message_link}}', label: 'Link to that message' },
      ];
    case 'member_joined':
      return [
        { token: '{{trigger.user}}', label: 'Person who joined' },
        { token: '{{trigger.channel}}', label: 'Channel' },
      ];
    case 'shortcut':
      return [
        { token: '{{trigger.user}}', label: 'Person who ran it' },
        { token: '{{trigger.channel}}', label: 'Channel' },
      ];
    case 'schedule':
      return [];
  }
}

function reorder(steps: Step[], order: number[]): Step[] {
  return order.map((old, at) => {
    const step = steps[old];
    if (step.type !== 'update_list_item') return step;
    const from = order.indexOf(step.from_step);
    return { ...step, from_step: from === -1 ? at : from };
  });
}

export function moveStep(steps: Step[], from: number, to: number): Step[] {
  if (to < 0 || to >= steps.length || from === to) return steps;
  const order = steps.map((_, i) => i);
  const [moved] = order.splice(from, 1);
  order.splice(to, 0, moved);
  return reorder(steps, order);
}

export function removeStep(steps: Step[], index: number): Step[] {
  return reorder(
    steps,
    steps.map((_, i) => i).filter((i) => i !== index),
  );
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

export function triggerSummary(
  trigger: Trigger,
  channelName: (id: string) => string | undefined = () => undefined,
): string {
  if (trigger.type === 'webhook') return 'Webhook';
  if (trigger.type !== 'schedule') {
    const name = channelName(trigger.channel_id);
    const where = name ? `#${name}` : 'a channel';
    if (trigger.type === 'shortcut') return `Shortcut in ${where}`;
    return trigger.type === 'reaction_added'
      ? `When ${trigger.emoji} is added in ${where}`
      : `When someone joins ${where}`;
  }
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

const STEP_LABELS: Record<Step['type'], string> = {
  send_message: 'Send a message',
  send_dm: 'Send a DM',
  form: 'Collect a form',
  add_list_item: 'Add a list item',
  update_list_item: 'Update the list item',
};

export function stepLabel(step: Step): string {
  return STEP_LABELS[step.type];
}

export function cleanDefinition(definition: Definition): Definition {
  const trigger =
    definition.trigger.type === 'webhook'
      ? { ...definition.trigger, variables: definition.trigger.variables.filter(Boolean) }
      : definition.trigger;
  const steps = definition.steps.map((step) =>
    step.type === 'form'
      ? {
          ...step,
          fields: step.fields.map((field) => ({
            ...field,
            options:
              field.kind === 'select'
                ? (field.options ?? []).map((option) => option.trim()).filter(Boolean)
                : [],
          })),
        }
      : step.type === 'add_list_item' || step.type === 'update_list_item'
        ? { ...step, cells: step.cells.filter((cell) => cell.value.trim()) }
        : step,
  );
  return { trigger, steps };
}

export function unpicked(definition: Definition): string | null {
  const trigger = definition.trigger;
  if ('channel_id' in trigger && !trigger.channel_id) return 'Pick the channel it listens to';
  for (const [index, step] of definition.steps.entries()) {
    const at = `Step ${index + 1}`;
    if (step.type === 'send_message' && !step.channel_id) return `${at}: pick a channel`;
    if ((step.type === 'send_dm' || step.type === 'form') && step.to.kind === 'person' && !step.to.user_id)
      return `${at}: pick a person`;
    if (step.type === 'add_list_item' && !step.list_id) return `${at}: pick a list`;
  }
  return null;
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
