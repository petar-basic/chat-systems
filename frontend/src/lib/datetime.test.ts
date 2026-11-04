import { describe, it, expect, beforeEach } from 'vitest';
import { useDisplayStore } from '@/stores/display';
import { TIME, formatClock, formatTime, localTime, offHours, timezoneChoices } from './datetime';

const at = new Date(Date.UTC(2026, 10, 4, 22, 5));

describe('the clock', () => {
  beforeEach(() => useDisplayStore.setState({ clock: 'h24' }));

  it('shows a 24-hour clock unless 12-hour was picked', () => {
    expect(formatClock(at, { ...TIME, timeZone: 'UTC' })).toMatch(/22:05/);
    expect(formatClock(at, { ...TIME, timeZone: 'UTC' }, 'h12')).toMatch(/10:05/);
    expect(formatClock(at, { ...TIME, timeZone: 'UTC' }, 'h12')).not.toMatch(/22|010/);
  });

  it('follows the preference kept in the display store', () => {
    useDisplayStore.setState({ clock: 'h12' });
    expect(formatTime(new Date(2026, 0, 1, 14, 5))).toMatch(/^2:05/);
    useDisplayStore.setState({ clock: 'h24' });
    expect(formatTime(new Date(2026, 0, 1, 14, 5))).toMatch(/14:05/);
  });
});

describe('somebody else’s time', () => {
  it('is shown in their time zone', () => {
    expect(localTime('Asia/Tokyo', at, 'h24')).toMatch(/07:05/);
    expect(localTime('Europe/Belgrade', at, 'h24')).toMatch(/23:05/);
  });

  it('is left out when the time zone is unknown or not set', () => {
    expect(localTime(null, at)).toBeNull();
    expect(localTime('Mars/Olympus_Mons', at)).toBeNull();
  });

  it('is early before 08:00 and late from 21:00', () => {
    expect(offHours('Europe/Belgrade', at)).toBe('late');
    expect(offHours('Asia/Tokyo', at)).toBe('early');
    expect(offHours('America/New_York', at)).toBeNull();
    expect(offHours('UTC', new Date(Date.UTC(2026, 10, 4, 21, 0)))).toBe('late');
    expect(offHours('UTC', new Date(Date.UTC(2026, 10, 4, 20, 59)))).toBeNull();
    expect(offHours('UTC', new Date(Date.UTC(2026, 10, 4, 8, 0)))).toBeNull();
    expect(offHours(undefined, at)).toBeNull();
  });
});

describe('timezoneChoices', () => {
  it('finds a zone by a word in its name, spaces or underscores alike', () => {
    expect(timezoneChoices('UTC', 'new york')).toContain('America/New_York');
    expect(timezoneChoices('UTC', 'tokyo')).toEqual(['UTC', 'Asia/Tokyo']);
  });

  it('keeps the current zone in the list whatever the search', () => {
    expect(timezoneChoices('Europe/Belgrade', 'tokyo')[0]).toBe('Europe/Belgrade');
  });
});
