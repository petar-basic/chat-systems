import { describe, it, expect } from 'vitest';
import {
  nextMorning,
  nextMondayMorning,
  SCHEDULE_PRESETS,
  toLocalInputValue,
  formatScheduleHint,
} from './schedulePresets';

describe('schedule presets', () => {
  it('moves to 9am on the following day', () => {
    const at = nextMorning(new Date('2026-03-10T22:30:00'), 1);

    expect(at.getDate()).toBe(11);
    expect(at.getHours()).toBe(9);
    expect(at.getMinutes()).toBe(0);
  });

  it('always lands on the next Monday, never today', () => {
    const fromMonday = nextMondayMorning(new Date('2026-03-09T08:00:00'));
    expect(fromMonday.getDay()).toBe(1);
    expect(fromMonday.getDate()).toBe(16);

    const fromFriday = nextMondayMorning(new Date('2026-03-13T08:00:00'));
    expect(fromFriday.getDay()).toBe(1);
    expect(fromFriday.getDate()).toBe(16);
  });

  it('offers three presets, all in the future', () => {
    const now = Date.now();
    expect(SCHEDULE_PRESETS).toHaveLength(3);
    for (const preset of SCHEDULE_PRESETS) {
      expect(preset.at().getTime()).toBeGreaterThan(now);
    }
  });
});

describe('custom schedule input', () => {
  it('formats a date for a datetime-local field in local time', () => {
    expect(toLocalInputValue(new Date(2026, 2, 9, 7, 5))).toBe('2026-03-09T07:05');
    expect(toLocalInputValue(new Date(2026, 11, 31, 23, 59))).toBe('2026-12-31T23:59');
  });

  it('round-trips through the input value without shifting the time', () => {
    const picked = new Date(2026, 5, 1, 14, 30);
    const parsed = new Date(toLocalInputValue(picked));

    expect(parsed.getTime()).toBe(picked.getTime());
  });

  it('hints just the time today and adds the day otherwise', () => {
    const now = new Date(2026, 2, 9, 8, 0);

    expect(formatScheduleHint(new Date(2026, 2, 9, 17, 30), now)).not.toMatch(/Mar/);
    expect(formatScheduleHint(new Date(2026, 2, 12, 9, 0), now)).toMatch(/Mar/);
  });
});
