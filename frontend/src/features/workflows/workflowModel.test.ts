import { describe, it, expect } from 'vitest';
import type { ListColumn } from '@/features/lists/listModel';
import {
  addStep,
  blankField,
  blankStep,
  flatten,
  inLane,
  opsFor,
  replaceStep,
  testFor,
  withOp,
  canBuild,
  cleanDefinition,
  fieldNameFrom,
  fits,
  listOf,
  matchColumns,
  removeStep,
  setCell,
  variableKind,
  relabel,
  stepVariables,
  daysFor,
  insertAt,
  moveStep,
  repeatOf,
  toggleDay,
  triggerPerson,
  triggerSummary,
  unpicked,
  variableNameError,
  variablesOf,
  workflowBuildersOf,
  type Definition,
  type FormStep,
  type Step,
} from './workflowModel';

describe('workflowModel', () => {
  it('names the usual schedules and keeps chosen days in week order', () => {
    expect(repeatOf(['mon', 'tue', 'wed', 'thu', 'fri'])).toBe('weekdays');
    expect(repeatOf(['sun', 'mon', 'tue', 'wed', 'thu', 'fri', 'sat'])).toBe('daily');
    expect(repeatOf(['mon', 'wed'])).toBe('custom');
    expect(daysFor('weekdays', ['sat'])).toEqual(['mon', 'tue', 'wed', 'thu', 'fri']);
    expect(daysFor('custom', ['sat'])).toEqual(['sat']);
    expect(toggleDay(['fri', 'mon'], 'wed')).toEqual(['mon', 'wed', 'fri']);
    expect(toggleDay(['mon', 'wed'], 'mon')).toEqual(['wed']);
  });

  it('summarises a trigger the way the list shows it', () => {
    expect(
      triggerSummary({
        type: 'schedule',
        days: ['mon', 'tue', 'wed', 'thu', 'fri'],
        time: '09:50',
        timezone: 'UTC',
      }),
    ).toBe('Weekdays at 09:50');
    expect(triggerSummary({ type: 'schedule', days: ['fri', 'mon'], time: '17:00', timezone: 'UTC' })).toBe(
      'Mon, Fri at 17:00',
    );
    expect(triggerSummary({ type: 'webhook', variables: [] })).toBe('Webhook');
  });

  it('summarises an event trigger with its channel', () => {
    const named = (id: string) => (id === 'c-1' ? 'incidents' : undefined);
    expect(triggerSummary({ type: 'reaction_added', channel_id: 'c-1', emoji: '🚨' }, named)).toBe(
      'When 🚨 is added in #incidents',
    );
    expect(triggerSummary({ type: 'member_joined', channel_id: 'c-1' }, named)).toBe(
      'When someone joins #incidents',
    );
    expect(triggerSummary({ type: 'member_joined', channel_id: 'gone' }, named)).toBe(
      'When someone joins a channel',
    );
  });

  it('offers the person and channel an event trigger brings, and a DM back to that person', () => {
    expect(variablesOf({ type: 'reaction_added', channel_id: 'c', emoji: '🚨' }).map((v) => v.token)).toEqual(
      ['{{trigger.user}}', '{{trigger.channel}}', '{{trigger.message}}', '{{trigger.message_link}}'],
    );
    expect(variablesOf({ type: 'member_joined', channel_id: 'c' }).map((v) => v.label)).toEqual([
      'Person who joined',
      'Channel',
    ]);
    expect(triggerPerson({ type: 'member_joined', channel_id: 'c' })).toBe('The person who joined');
    expect(triggerPerson({ type: 'webhook', variables: [] })).toBeNull();
  });

  it('offers a webhook’s declared variables and nothing for a schedule', () => {
    expect(variablesOf({ type: 'webhook', variables: ['service', '', 'code'] })).toEqual([
      { token: '{{webhook.service}}', label: 'service' },
      { token: '{{webhook.code}}', label: 'code' },
    ]);
    expect(variablesOf({ type: 'schedule', days: ['mon'], time: '09:00', timezone: 'UTC' })).toEqual([]);
  });

  it('inserts a variable at the caret, replacing a selection', () => {
    expect(insertAt('down: ', '{{webhook.service}}', 6)).toEqual({
      text: 'down: {{webhook.service}}',
      caret: 25,
    });
    expect(insertAt('hello NAME!', '{{webhook.name}}', 6, 10)).toEqual({
      text: 'hello {{webhook.name}}!',
      caret: 22,
    });
    expect(insertAt('x', '{{a}}', 99).text).toBe('x{{a}}');
  });

  it('moves a step and refuses to move one past either end', () => {
    const steps: Step[] = ['one', 'two', 'three'].map((text) => ({
      type: 'send_message',
      channel_id: 'c',
      text,
    }));
    expect(moveStep(steps, 2, -2).map((s) => ('text' in s ? s.text : s.type))).toEqual([
      'three',
      'one',
      'two',
    ]);
    expect(moveStep(steps, 0, -1)).toBe(steps);
    expect(moveStep(steps, 2, 1)).toBe(steps);
  });

  it('checks a variable name the way the server does', () => {
    expect(variableNameError('')).toBeNull();
    expect(variableNameError('status_code')).toBeNull();
    expect(variableNameError('Status')).not.toBeNull();
    expect(variableNameError('1st')).not.toBeNull();
    expect(variableNameError('a'.repeat(41))).toBe('At most 40 characters');
  });

  it('lets members build unless the workspace keeps it to admins, and never guests', () => {
    expect(workflowBuildersOf({ workflow_builders: 'admins' })).toBe('admins');
    expect(workflowBuildersOf({})).toBe('members');
    expect(workflowBuildersOf(null)).toBe('members');
    expect(canBuild('member', 'members')).toBe(true);
    expect(canBuild('member', 'admins')).toBe(false);
    expect(canBuild('admin', 'admins')).toBe(true);
    expect(canBuild('guest', 'members')).toBe(false);
    expect(canBuild(null, 'members')).toBe(false);
  });

  it('offers a shortcut the person who ran it and the answers of every earlier form', () => {
    const trigger = { type: 'shortcut', channel_id: 'c' } as const;
    expect(triggerPerson(trigger)).toBe('The person who ran it');
    expect(triggerSummary(trigger, () => 'help')).toBe('Shortcut in #help');
    const form: FormStep = {
      type: 'form',
      to: { kind: 'trigger_user' },
      title: 'Time off',
      fields: [
        { name: 'reason', label: 'Reason', kind: 'long_text' },
        { name: 'from', label: '', kind: 'date' },
      ],
    };
    const definition: Definition = {
      trigger,
      steps: [
        { type: 'send_message', channel_id: 'c', text: 'x' },
        form,
        { type: 'send_message', channel_id: 'c', text: 'y' },
      ],
    };
    expect(stepVariables(definition, 1).map((v) => v.token)).toEqual([
      '{{trigger.user}}',
      '{{trigger.channel}}',
    ]);
    expect(stepVariables(definition, 2).slice(2)).toEqual([
      { token: '{{form.reason}}', label: 'Answer: Reason' },
      { token: '{{form.from}}', label: 'Answer: from' },
    ]);
  });

  it('names a question after its label until someone renames it', () => {
    expect(fieldNameFrom('How many days?', [])).toBe('how_many_days');
    expect(fieldNameFrom('2nd choice', [])).toBe('nd_choice');
    expect(fieldNameFrom('', ['answer'])).toBe('answer_2');
    let fields = [blankField([])];
    fields = relabel(fields, 0, 'Reason');
    expect(fields[0].name).toBe('reason');
    fields = relabel([...fields, blankField(fields)], 1, 'Reason');
    expect(fields.map((f) => f.name)).toEqual(['reason', 'reason_2']);
    const renamed = relabel([{ ...fields[0], name: 'why' }], 0, 'Why exactly');
    expect(renamed[0].name).toBe('why');
  });

  it('drops blank choices and webhook variables before saving', () => {
    const cleaned = cleanDefinition({
      trigger: { type: 'webhook', variables: ['a', ''] },
      steps: [
        {
          type: 'form',
          to: { kind: 'person', user_id: 'u' },
          title: 'T',
          fields: [
            { name: 'pick', label: 'Pick', kind: 'select', options: [' Red ', '', 'Blue'] },
            { name: 'note', label: 'Note', kind: 'short_text', options: ['left over'] },
          ],
        },
      ],
    });
    expect(cleaned.trigger).toEqual({ type: 'webhook', variables: ['a'] });
    const step = cleaned.steps[0] as FormStep;
    expect(step.fields.map((f) => f.options)).toEqual([['Red', 'Blue'], []]);
  });

  it('keeps an update step pointing at the step that adds its item when steps move', () => {
    const steps: Step[] = [
      { type: 'send_message', channel_id: 'c', text: 'hi' },
      { type: 'add_list_item', list_id: 'l', cells: [] },
      { type: 'update_list_item', from_step: 1, cells: [] },
    ];
    expect(moveStep(steps, 0, 2)[1]).toEqual({ type: 'update_list_item', from_step: 0, cells: [] });
    expect(listOf(moveStep(steps, 0, 2), 1)).toBe('l');
    expect(removeStep(steps, 0)[1]).toEqual({ type: 'update_list_item', from_step: 0, cells: [] });
    expect(removeStep(steps, 1)[1]).toEqual({ type: 'update_list_item', from_step: 1, cells: [] });
    expect(listOf(removeStep(steps, 1), 1)).toBeNull();
  });

  it('offers a column only the variables it can hold and fills columns named like a question', () => {
    const column = (name: string, kind: ListColumn['kind'], choices: string[] = []): ListColumn => ({
      id: name,
      list_id: 'l',
      name,
      kind,
      position: 0,
      hidden: false,
      due: false,
      options: { choices: choices.map((label) => ({ id: `${name}-${label}`, label, color: 'gray' })) },
    });
    const definition: Definition = {
      trigger: { type: 'shortcut', channel_id: 'c' },
      steps: [
        {
          type: 'form',
          to: { kind: 'trigger_user' },
          title: 'Time off',
          fields: [
            { name: 'kind', label: 'Kind', kind: 'select', options: ['Sick', 'holiday'] },
            { name: 'from', label: 'From', kind: 'date' },
            { name: 'note', label: 'Note', kind: 'short_text' },
          ],
        },
        { type: 'add_list_item', list_id: 'l', cells: [] },
      ],
    };
    const kind = column('Kind', 'select', ['Sick', 'Holiday']);
    expect(variableKind(definition, 1, '{{trigger.user}}')).toEqual({ kind: 'person' });
    expect(fits(column('Who', 'person'), variableKind(definition, 1, '{{trigger.user}}'))).toBe(true);
    expect(fits(column('Who', 'person'), variableKind(definition, 1, '{{form.from}}'))).toBe(false);
    expect(fits(kind, variableKind(definition, 1, '{{form.kind}}'))).toBe(true);
    expect(fits(column('Kind', 'select', ['Sick']), variableKind(definition, 1, '{{form.kind}}'))).toBe(
      false,
    );
    expect(fits(column('Days', 'number'), variableKind(definition, 1, '{{form.note}}'))).toBe(true);
    expect(matchColumns([kind, column('From', 'date'), column('Days', 'number')], definition, 1)).toEqual([
      { column_id: 'Kind', value: '{{form.kind}}' },
      { column_id: 'From', value: '{{form.from}}' },
    ]);
    const cells = setCell(setCell([], 'a', 'x'), 'b', 'y');
    expect(setCell(cells, 'a', 'z')).toEqual([
      { column_id: 'a', value: 'z' },
      { column_id: 'b', value: 'y' },
    ]);
    expect(setCell(cells, 'a', '')).toEqual([{ column_id: 'b', value: 'y' }]);
  });

  it('names the first step still missing its channel, person or list', () => {
    const steps: Step[] = [
      { type: 'send_message', channel_id: 'c', text: 'hi' },
      { type: 'form', to: { kind: 'trigger_user' }, title: 'T', fields: [] },
      { type: 'add_list_item', list_id: '', cells: [] },
    ];
    expect(unpicked({ trigger: { type: 'shortcut', channel_id: '' }, steps })).toBe(
      'Pick the channel it listens to',
    );
    expect(unpicked({ trigger: { type: 'shortcut', channel_id: 'c' }, steps })).toBe('Step 3: pick a list');
    expect(
      unpicked({
        trigger: { type: 'webhook', variables: [] },
        steps: [{ type: 'send_dm', to: { kind: 'person', user_id: '' }, text: 'x' }],
      }),
    ).toBe('Step 1: pick a person');
    expect(unpicked({ trigger: { type: 'webhook', variables: [] }, steps: steps.slice(0, 2) })).toBeNull();
  });

  const say = (text: string): Step => ({ type: 'send_message', channel_id: 'c', text });
  const branch = (then: Step[], otherwise: Step[] = [], value = 'form.urgency'): Step => ({
    type: 'branch',
    value,
    test: { op: 'equals', value: 'urgent' },
    then,
    otherwise,
  });
  const ask = (name: string, kind: FormStep['fields'][number]['kind'] = 'short_text'): Step => ({
    type: 'form',
    to: { kind: 'trigger_user' },
    title: 'Ask',
    fields: [{ name, label: name, kind, options: [], required: true }],
  });
  const textOf = (step: Step) => ('text' in step ? step.text : step.type);

  it('numbers the steps of a branch the way the server does, each path seeing what came before it', () => {
    const steps = [ask('urgency'), branch([say('page'), ask('pager')], [say('file')]), say('done')];
    const flat = flatten(steps);
    expect(flat.map((node) => textOf(node.step))).toEqual(['form', 'branch', 'page', 'form', 'file', 'done']);
    expect(flat.map((node) => node.depth)).toEqual([0, 0, 1, 1, 1, 0]);
    expect(flat[4].before).toEqual([0, 1]);
    expect(flat[5].before).toEqual([0, 1]);
    expect(flat[3].path).toEqual([[1, true]]);
    expect(inLane(flat, [1, false]).map((node) => node.position)).toEqual([4]);
    expect(inLane(flat, null).map((node) => node.position)).toEqual([0, 1, 5]);

    const definition: Definition = { trigger: { type: 'shortcut', channel_id: 'c' }, steps };
    expect(stepVariables(definition, 3).map((v) => v.token)).toContain('{{form.urgency}}');
    expect(stepVariables(definition, 4).map((v) => v.token)).not.toContain('{{form.pager}}');
    expect(stepVariables(definition, 5).map((v) => v.token)).not.toContain('{{form.pager}}');
  });

  it('offers what a created channel and a thread reply leave behind to the steps after them', () => {
    const definition: Definition = {
      trigger: { type: 'reaction_added', channel_id: 'c', emoji: '👀' },
      steps: [
        { type: 'create_channel', name: 'proj', private: false },
        { type: 'reply_in_thread', text: 'Made {{step.1.channel}}' },
        say('end'),
      ],
    };
    expect(stepVariables(definition, 0).map((v) => v.token)).not.toContain('{{step.1.channel}}');
    expect(stepVariables(definition, 2).slice(-3)).toEqual([
      { token: '{{step.1.channel}}', label: 'Step 1: the new channel' },
      { token: '{{step.1.channel_link}}', label: 'Step 1: link to the new channel' },
      { token: '{{step.2.message_link}}', label: 'Step 2: link to the reply' },
    ]);
  });

  it('keeps every reference on its step when a step is added, moved or removed around it', () => {
    const steps: Step[] = [
      { type: 'create_channel', name: 'proj', private: false },
      { type: 'add_list_item', list_id: 'l', cells: [] },
      branch([say('a')], [], 'step.2.column.col'),
      {
        type: 'update_list_item',
        from_step: 1,
        cells: [{ column_id: 'x', value: '{{step.1.channel_link}}' }],
      },
      { type: 'set_topic', channel: { kind: 'from_step', step: 0 }, topic: 'In {{step.1.channel}}' },
    ];
    const added = addStep(steps, [2, true], say('b'));
    expect(flatten(added).map((node) => textOf(node.step))).toEqual([
      'create_channel',
      'add_list_item',
      'branch',
      'a',
      'b',
      'update_list_item',
      'set_topic',
    ]);
    expect(added[3]).toEqual({ ...steps[3], from_step: 1 });

    const moved = moveStep(steps, 0, 1);
    expect(moved[2]).toMatchObject({ value: 'step.1.column.col' });
    expect(moved[3]).toEqual({
      type: 'update_list_item',
      from_step: 0,
      cells: [{ column_id: 'x', value: '{{step.2.channel_link}}' }],
    });
    expect(moved[4]).toEqual({
      type: 'set_topic',
      channel: { kind: 'from_step', step: 1 },
      topic: 'In {{step.2.channel}}',
    });

    const removed = removeStep(steps, 0);
    expect(removed[3]).toEqual({
      type: 'set_topic',
      channel: { kind: 'from_step', step: 4 },
      topic: 'In {{step.5.channel}}',
    });
    expect(removed[1]).toMatchObject({ value: 'step.1.column.col' });
  });

  it('moves a branch with its paths and drops them when it stops being a branch', () => {
    const steps = [say('one'), branch([say('a')], [say('b')]), say('two')];
    expect(flatten(moveStep(steps, 1, -1)).map((node) => textOf(node.step))).toEqual([
      'branch',
      'a',
      'b',
      'one',
      'two',
    ]);
    expect(moveStep(steps, 2, 1)).toBe(steps);
    const condition = { ...(steps[1] as Extract<Step, { type: 'branch' }>), then: [], otherwise: [] };
    expect(flatten(replaceStep(steps, 1, { ...condition, value: 'form.x' })).length).toBe(5);
    expect(replaceStep(steps, 1, say('flat'))).toEqual([say('one'), say('flat'), say('two')]);
    expect(blankStep('branch')).toEqual({
      type: 'branch',
      value: '',
      test: { op: 'equals', value: '' },
      then: [],
      otherwise: [],
    });
  });

  it('tests a person, a choice or a date only in ways that make sense for it', () => {
    expect(opsFor({ kind: 'person' })).toEqual(['equals', 'one_of', 'is_empty']);
    expect(opsFor({ kind: 'date' })).not.toContain('contains');
    expect(withOp({ op: 'equals', value: 'a' }, 'one_of')).toEqual({ op: 'one_of', values: ['a'] });
    expect(withOp({ op: 'one_of', values: ['a', 'b'] }, 'contains')).toEqual({ op: 'contains', value: 'a' });
    expect(withOp({ op: 'contains', value: 'a' }, 'is_empty')).toEqual({ op: 'is_empty' });
    expect(testFor({ op: 'contains', value: 'x' }, { kind: 'person' })).toEqual({ op: 'equals', value: 'x' });
  });

  it('finds what is unpicked inside a branch and cleans its values', () => {
    const steps = [say('one'), branch([{ type: 'add_reaction', emoji: '' }])];
    const trigger = { type: 'reaction_added' as const, channel_id: 'c', emoji: '👀' };
    expect(unpicked({ trigger, steps })).toBe('Step 3: pick an emoji');
    expect(unpicked({ trigger, steps: [say('one'), branch([], [], '')] })).toBe(
      'Step 2: pick what the branch tests',
    );
    expect(unpicked({ trigger, steps: [branch([])] })).toBe('Step 1: add a step to one of its branches');
    expect(
      unpicked({
        trigger,
        steps: [{ type: 'archive_channel', channel: { kind: 'channel', channel_id: '' } }],
      }),
    ).toBe('Step 1: pick a channel');
    const cleaned = cleanDefinition({
      trigger,
      steps: [
        {
          ...(branch([say('x')]) as Extract<Step, { type: 'branch' }>),
          test: { op: 'one_of', values: [' a', '', 'b '] },
        },
      ],
    });
    expect(cleaned.steps[0]).toMatchObject({ test: { op: 'one_of', values: ['a', 'b'] } });
  });
});
