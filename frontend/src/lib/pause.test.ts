import { describe, it, expect } from 'vitest';
import { currentPause, pauseLabel } from './pause';

const now = Date.parse('2026-10-21T18:00:00Z');
const iso = (offsetMinutes: number) => new Date(now + offsetMinutes * 60000).toISOString();

describe('currentPause', () => {
  it('is nothing when neither the timer nor the schedule is running', () => {
    expect(currentPause(undefined, now)).toBeNull();
    expect(currentPause({ dnd_until: iso(-5), schedule_paused_until: null }, now)).toBeNull();
  });

  it('names the schedule when it is what keeps notifications paused', () => {
    expect(currentPause({ dnd_until: null, schedule_paused_until: iso(600) }, now)).toEqual({
      until: iso(600),
      bySchedule: true,
    });
    expect(currentPause({ dnd_until: iso(30), schedule_paused_until: iso(600) }, now)).toEqual({
      until: iso(600),
      bySchedule: true,
    });
  });

  it('names the timer when it outlasts the schedule', () => {
    expect(currentPause({ dnd_until: iso(900), schedule_paused_until: iso(600) }, now)).toEqual({
      until: iso(900),
      bySchedule: false,
    });
  });
});

describe('pauseLabel', () => {
  it('says why notifications are paused', () => {
    const today = new Date(now);
    expect(pauseLabel({ until: iso(60), bySchedule: true }, today)).toMatch(
      /^Paused by your schedule until /,
    );
    expect(pauseLabel({ until: iso(60), bySchedule: false }, today)).toMatch(/^Paused until /);
  });
});
