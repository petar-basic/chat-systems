import type { components } from '@/api/schema';
import type { ListColumn } from '@/features/lists';
import { allows, ruleOf, type WhoMay } from '@/lib/workspaceRules';

export type Workflow = components['schemas']['Workflow'];
export type WorkflowRun = components['schemas']['WorkflowRun'];
export type Definition = components['schemas']['Definition'];
export type Trigger = components['schemas']['Trigger'];
export type Step = components['schemas']['Step'];
export type Weekday = components['schemas']['Weekday'];
export type SaveWorkflow = components['schemas']['SaveWorkflowRequest'];
export type WorkflowBuilders = WhoMay;
export type FormField = components['schemas']['FormField'];
export type FieldKind = components['schemas']['FieldKind'];
export type FormStep = Extract<Step, { type: 'form' }>;
export type TextStep = Extract<Step, { type: 'send_message' | 'send_dm' | 'reply_in_thread' }>;
export type AddItemStep = Extract<Step, { type: 'add_list_item' }>;
export type UpdateItemStep = Extract<Step, { type: 'update_list_item' }>;
export type ListStep = AddItemStep | UpdateItemStep;
export type BranchStep = Extract<Step, { type: 'branch' }>;
export type DelayStep = Extract<Step, { type: 'delay' }>;
export type ChannelStep = Extract<Step, { type: 'add_to_channel' | 'set_topic' | 'archive_channel' }>;
export type Test = components['schemas']['Test'];
export type TestOp = Test['op'];
export type Wait = components['schemas']['Wait'];
export type DelayUnit = components['schemas']['DelayUnit'];
export type ChannelRef = components['schemas']['ChannelRef'];
export type Invitee = components['schemas']['Invitee'];
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
export const MAX_BRANCH_DEPTH = 2;
export const MAX_DELAY_DAYS = 30;
export const MAX_TEST_VALUES = 20;
export const TEST_VALUE_MAX = 200;
export const MAX_INVITEES = 10;
export const CHANNEL_NAME_MAX = 80;
export const TOPIC_MAX = 500;

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

export function blankStep(
  type: Step['type'],
  channelId = '',
  fromStep = 0,
  channel: ChannelRef = { kind: 'channel', channel_id: channelId },
): Step {
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
    case 'branch':
      return { type: 'branch', value: '', test: { op: 'equals', value: '' }, then: [], otherwise: [] };
    case 'delay':
      return { type: 'delay', wait: { kind: 'for', amount: 1, unit: 'days' } };
    case 'add_reaction':
      return { type: 'add_reaction', emoji: '' };
    case 'reply_in_thread':
      return { type: 'reply_in_thread', text: '' };
    case 'create_channel':
      return { type: 'create_channel', name: '', private: false };
    case 'add_to_channel':
      return { type: 'add_to_channel', channel, people: [] };
    case 'set_topic':
      return { type: 'set_topic', channel, topic: '' };
    case 'archive_channel':
      return { type: 'archive_channel', channel };
  }
}

export type Lane = [branch: number, matched: boolean];

export interface FlatStep {
  step: Step;
  position: number;
  depth: number;
  path: Lane[];
  before: number[];
  index: number;
  siblings: number;
}

export function flatten(steps: Step[]): FlatStep[] {
  const nodes: FlatStep[] = [];
  const walk = (lane: Step[], depth: number, path: Lane[], inherited: number[]) => {
    const before = [...inherited];
    lane.forEach((step, index) => {
      const position = nodes.length;
      nodes.push({ step, position, depth, path, before: [...before], index, siblings: lane.length });
      before.push(position);
      if (step.type === 'branch') {
        walk(step.then, depth + 1, [...path, [position, true]], before);
        walk(step.otherwise, depth + 1, [...path, [position, false]], before);
      }
    });
  };
  walk(steps, 0, [], []);
  return nodes;
}

export function inLane(flat: FlatStep[], lane: Lane | null): FlatStep[] {
  return flat.filter((node) => {
    const last = node.path[node.path.length - 1];
    return lane ? last?.[0] === lane[0] && last[1] === lane[1] : !last;
  });
}

export function stepsBefore(steps: Step[], position: number): FlatStep[] {
  const flat = flatten(steps);
  return (flat[position]?.before ?? []).map((at) => flat[at]);
}

interface Tagged {
  step: Step;
  old: number;
  then: Tagged[];
  otherwise: Tagged[];
}

function fresh(step: Step): Tagged {
  return step.type === 'branch'
    ? { step, old: -1, then: step.then.map(fresh), otherwise: step.otherwise.map(fresh) }
    : { step, old: -1, then: [], otherwise: [] };
}

function tagged(steps: Step[]): Tagged[] {
  let next = 0;
  const tag = (lane: Step[]): Tagged[] =>
    lane.map((step) => {
      const old = next++;
      return step.type === 'branch'
        ? { step, old, then: tag(step.then), otherwise: tag(step.otherwise) }
        : { step, old, then: [], otherwise: [] };
    });
  return tag(steps);
}

function renumbered(step: Step, to: (old: number) => number): Step {
  const tokens = (text: string) =>
    text.replace(/(\{\{\s*step\.)(\d+)\./g, (match, head: string, n: string) =>
      Number(n) > 0 ? `${head}${to(Number(n) - 1) + 1}.` : match,
    );
  const channel = (ref: ChannelRef): ChannelRef =>
    ref.kind === 'from_step' ? { kind: 'from_step', step: to(ref.step) } : ref;
  const cells = (list: CellValue[]) => list.map((cell) => ({ ...cell, value: tokens(cell.value) }));
  switch (step.type) {
    case 'send_message':
    case 'send_dm':
    case 'reply_in_thread':
      return { ...step, text: tokens(step.text) };
    case 'add_list_item':
      return { ...step, cells: cells(step.cells) };
    case 'update_list_item':
      return { ...step, from_step: to(step.from_step), cells: cells(step.cells) };
    case 'branch':
      return {
        ...step,
        value: step.value.replace(/^(step\.)(\d+)\./, (match, head: string, n: string) =>
          Number(n) > 0 ? `${head}${to(Number(n) - 1) + 1}.` : match,
        ),
      };
    case 'create_channel':
      return { ...step, name: tokens(step.name) };
    case 'set_topic':
      return { ...step, channel: channel(step.channel), topic: tokens(step.topic) };
    case 'add_to_channel':
    case 'archive_channel':
      return { ...step, channel: channel(step.channel) };
    default:
      return step;
  }
}

function untagged(tree: Tagged[]): Step[] {
  const order: number[] = [];
  const walk = (lane: Tagged[]) =>
    lane.forEach((node) => {
      order.push(node.old);
      walk(node.then);
      walk(node.otherwise);
    });
  walk(tree);
  let next = 0;
  const build = (lane: Tagged[]): Step[] =>
    lane.map((node) => {
      const self = next++;
      const step = renumbered(node.step, (old) => {
        const at = order.indexOf(old);
        return at === -1 ? self : at;
      });
      if (step.type !== 'branch') return step;
      const then = build(node.then);
      return { ...step, then, otherwise: build(node.otherwise) };
    });
  return build(tree);
}

function located(tree: Tagged[], position: number): { lane: Tagged[]; index: number } | null {
  for (const [index, node] of tree.entries()) {
    if (node.old === position) return { lane: tree, index };
    const inner = located(node.then, position) ?? located(node.otherwise, position);
    if (inner) return inner;
  }
  return null;
}

function edited(steps: Step[], change: (tree: Tagged[]) => boolean): Step[] {
  const tree = tagged(steps);
  return change(tree) ? untagged(tree) : steps;
}

export function addStep(steps: Step[], lane: Lane | null, step: Step): Step[] {
  return edited(steps, (tree) => {
    const found = lane ? located(tree, lane[0]) : null;
    const branch = found?.lane[found.index];
    const target = lane ? (branch ? (lane[1] ? branch.then : branch.otherwise) : null) : tree;
    target?.push(fresh(step));
    return Boolean(target);
  });
}

export function replaceStep(steps: Step[], position: number, step: Step): Step[] {
  return edited(steps, (tree) => {
    const found = located(tree, position);
    if (!found) return false;
    const current = found.lane[found.index];
    found.lane[found.index] =
      step.type !== 'branch'
        ? { step, old: position, then: [], otherwise: [] }
        : current.step.type === 'branch'
          ? { ...current, step }
          : { ...fresh(step), old: position };
    return true;
  });
}

export function moveStep(steps: Step[], position: number, offset: number): Step[] {
  return edited(steps, (tree) => {
    const found = located(tree, position);
    const to = (found?.index ?? 0) + offset;
    if (!found || offset === 0 || to < 0 || to >= found.lane.length) return false;
    const [moved] = found.lane.splice(found.index, 1);
    found.lane.splice(to, 0, moved);
    return true;
  });
}

export function removeStep(steps: Step[], position: number): Step[] {
  return edited(steps, (tree) => {
    const found = located(tree, position);
    found?.lane.splice(found.index, 1);
    return Boolean(found);
  });
}

export function addStepsBefore(steps: Step[], position: number): number[] {
  return stepsBefore(steps, position).flatMap((node) =>
    node.step.type === 'add_list_item' ? [node.position] : [],
  );
}

export function channelStepsBefore(steps: Step[], position: number): number[] {
  return stepsBefore(steps, position).flatMap((node) =>
    node.step.type === 'create_channel' ? [node.position] : [],
  );
}

export function listStepsBefore(steps: Step[], position: number): { position: number; listId: string }[] {
  return stepsBefore(steps, position).flatMap((node) => {
    const listId = listOf(steps, node.position);
    return listId ? [{ position: node.position, listId }] : [];
  });
}

export function listOf(steps: Step[], position: number): string | null {
  const flat = flatten(steps);
  const find = (at: number): string | null => {
    const node = flat[at];
    if (node?.step.type === 'add_list_item') return node.step.list_id || null;
    if (node?.step.type === 'update_list_item' && node.before.includes(node.step.from_step))
      return find(node.step.from_step);
    return null;
  };
  return find(position);
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
  return stepsBefore(steps, index)
    .map((node) => node.step)
    .filter((step): step is FormStep => step.type === 'form');
}

export function personQuestionsBefore(steps: Step[], index: number): FormField[] {
  return formsBefore(steps, index).flatMap((form) =>
    form.fields.filter((field) => field.name && field.kind === 'person'),
  );
}

export function stepVariables(definition: Definition, index: number): Variable[] {
  const earlier = stepsBefore(definition.steps, index).flatMap(({ step, position }): Variable[] => {
    const at = `Step ${position + 1}`;
    if (step.type === 'form')
      return step.fields
        .filter((field) => field.name)
        .map((field) => ({ token: `{{form.${field.name}}}`, label: `Answer: ${field.label || field.name}` }));
    if (step.type === 'create_channel')
      return [
        { token: `{{step.${position + 1}.channel}}`, label: `${at}: the new channel` },
        { token: `{{step.${position + 1}.channel_link}}`, label: `${at}: link to the new channel` },
      ];
    if (step.type === 'reply_in_thread')
      return [{ token: `{{step.${position + 1}.message_link}}`, label: `${at}: link to the reply` }];
    return [];
  });
  return [...variablesOf(definition.trigger), ...earlier];
}

export function branchVariables(definition: Definition, index: number): Variable[] {
  return stepVariables(definition, index)
    .filter((variable) => !variable.token.startsWith('{{step.'))
    .map((variable) => ({ ...variable, token: variable.token.slice(2, -2) }));
}

export const TEST_OPS: TestOp[] = ['equals', 'one_of', 'contains', 'greater_than', 'is_empty'];

export function opsFor(kind: VariableKind): TestOp[] {
  if (kind.kind === 'person') return ['equals', 'one_of', 'is_empty'];
  if (kind.kind === 'choice') return ['equals', 'one_of', 'contains', 'is_empty'];
  if (kind.kind === 'date') return ['equals', 'greater_than', 'is_empty'];
  return TEST_OPS;
}

export function opLabel(op: TestOp, kind: VariableKind): string {
  switch (op) {
    case 'equals':
      return 'is';
    case 'one_of':
      return 'is one of';
    case 'contains':
      return 'contains';
    case 'greater_than':
      return kind.kind === 'date' ? 'is after' : 'is greater than';
    case 'is_empty':
      return 'is empty';
  }
}

export function operandsOf(test: Test): string[] {
  if (test.op === 'one_of') return test.values;
  if (test.op === 'is_empty') return [];
  return [test.value];
}

export function withOp(test: Test, op: TestOp): Test {
  const [first = ''] = operandsOf(test);
  if (op === 'is_empty') return { op };
  if (op === 'one_of') return { op, values: test.op === 'one_of' ? test.values : first ? [first] : [] };
  return { op, value: first };
}

export function emptyTest(op: TestOp): Test {
  if (op === 'is_empty') return { op };
  if (op === 'one_of') return { op, values: [] };
  return { op, value: '' };
}

export function blankWait(kind: Wait['kind']): Wait {
  if (kind === 'for') return { kind: 'for', amount: 1, unit: 'days' };
  const tomorrow = new Date(Date.now() + 24 * 60 * 60 * 1000);
  return {
    kind: 'until',
    date: tomorrow.toLocaleDateString('en-CA'),
    time: '09:00',
    timezone: localTimezone(),
  };
}

export function testFor(test: Test, kind: VariableKind): Test {
  const ops = opsFor(kind);
  return ops.includes(test.op) ? test : withOp(test, ops[0]);
}

export function channelRefFor(definition: Definition, index: number): ChannelRef {
  const created = channelStepsBefore(definition.steps, index);
  if (created.length > 0) return { kind: 'from_step', step: created[created.length - 1] };
  if ('channel_id' in definition.trigger) return { kind: 'trigger_channel' };
  return { kind: 'channel', channel_id: '' };
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
  branch: 'Branch',
  delay: 'Wait',
  add_reaction: 'Add a reaction',
  reply_in_thread: 'Reply in the thread',
  create_channel: 'Create a channel',
  add_to_channel: 'Add people to a channel',
  set_topic: 'Set a channel topic',
  archive_channel: 'Archive a channel',
};

export function stepLabel(step: Step): string {
  return STEP_LABELS[step.type];
}

function cleanSteps(steps: Step[]): Step[] {
  return steps.map((step) => {
    switch (step.type) {
      case 'form':
        return {
          ...step,
          fields: step.fields.map((field) => ({
            ...field,
            options:
              field.kind === 'select'
                ? (field.options ?? []).map((option) => option.trim()).filter(Boolean)
                : [],
          })),
        };
      case 'add_list_item':
      case 'update_list_item':
        return { ...step, cells: step.cells.filter((cell) => cell.value.trim()) };
      case 'branch':
        return {
          ...step,
          test:
            step.test.op === 'one_of'
              ? { ...step.test, values: step.test.values.map((v) => v.trim()).filter(Boolean) }
              : step.test,
          then: cleanSteps(step.then),
          otherwise: cleanSteps(step.otherwise),
        };
      default:
        return step;
    }
  });
}

export function cleanDefinition(definition: Definition): Definition {
  const trigger =
    definition.trigger.type === 'webhook'
      ? { ...definition.trigger, variables: definition.trigger.variables.filter(Boolean) }
      : definition.trigger;
  return { trigger, steps: cleanSteps(definition.steps) };
}

function missing(step: Step): string | null {
  switch (step.type) {
    case 'send_message':
      return step.channel_id ? null : 'pick a channel';
    case 'send_dm':
    case 'form':
      return step.to.kind === 'person' && !step.to.user_id ? 'pick a person' : null;
    case 'add_list_item':
      return step.list_id ? null : 'pick a list';
    case 'branch':
      if (!step.value) return 'pick what the branch tests';
      return step.then.length + step.otherwise.length ? null : 'add a step to one of its branches';
    case 'add_reaction':
      return step.emoji ? null : 'pick an emoji';
    case 'add_to_channel':
    case 'set_topic':
    case 'archive_channel':
      if (step.channel.kind === 'channel' && !step.channel.channel_id) return 'pick a channel';
      return step.type === 'add_to_channel' && step.people.length === 0 ? 'pick who to add' : null;
    default:
      return null;
  }
}

export function unpicked(definition: Definition): string | null {
  const trigger = definition.trigger;
  if ('channel_id' in trigger && !trigger.channel_id) return 'Pick the channel it listens to';
  for (const node of flatten(definition.steps)) {
    const problem = missing(node.step);
    if (problem) return `Step ${node.position + 1}: ${problem}`;
  }
  return null;
}

export function workflowBuildersOf(settings: unknown): WorkflowBuilders {
  return ruleOf(settings, 'workflow_builders');
}

export function canBuild(role: string | null, builders: WorkflowBuilders): boolean {
  return allows(builders, role);
}

export function canManageAll(role: string | null): boolean {
  return role === 'admin' || role === 'owner';
}
