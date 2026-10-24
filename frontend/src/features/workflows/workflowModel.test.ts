import { describe, it, expect } from 'vitest';
import type { ListColumn } from '@/features/lists/listModel';
import {
  blankField,
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
    expect(moveStep(steps, 2, 0).map((s) => ('text' in s ? s.text : s.type))).toEqual([
      'three',
      'one',
      'two',
    ]);
    expect(moveStep(steps, 0, -1)).toBe(steps);
    expect(moveStep(steps, 2, 3)).toBe(steps);
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
});
