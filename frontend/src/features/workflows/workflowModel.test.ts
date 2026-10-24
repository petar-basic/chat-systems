import { describe, it, expect } from 'vitest';
import {
  blankStep,
  canBuild,
  daysFor,
  insertAt,
  moveStep,
  repeatOf,
  toggleDay,
  triggerSummary,
  variableNameError,
  variablesOf,
  workflowBuildersOf,
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
    const steps: Step[] = [
      { ...blankStep('send_message', 'c'), text: 'one' },
      { ...blankStep('send_message', 'c'), text: 'two' },
      { ...blankStep('send_message', 'c'), text: 'three' },
    ];
    expect(moveStep(steps, 2, 0).map((s) => s.text)).toEqual(['three', 'one', 'two']);
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
});
