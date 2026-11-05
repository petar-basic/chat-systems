import { describe, it, expect, beforeEach } from 'vitest';
import { useDisplayStore } from '@/stores/display';
import {
  TIME,
  formatClock,
  formatTime,
  localTime,
  matchesTimezone,
  offHours,
  timezoneChoices,
} from './datetime';

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
  const winter = new Date(Date.UTC(2026, 0, 15, 12));
  const zones = timezoneChoices('Europe/Belgrade', winter);
  const find = (zone: string) => zones.find((choice) => choice.zone === zone);

  it('names a zone by its city and region, with its offset at that moment', () => {
    expect(find('Europe/Belgrade')).toEqual({
      zone: 'Europe/Belgrade',
      city: 'Belgrade',
      region: 'Europe',
      offset: 'GMT+1',
      minutes: 60,
    });
    expect(find('America/Argentina/La_Rioja')?.region).toBe('America · Argentina');
    expect(find('Asia/Kolkata')?.offset ?? find('Asia/Calcutta')?.offset).toBe('GMT+5:30');
  });

  it('leaves out the Etc zones and keeps one UTC', () => {
    expect(zones.some((choice) => choice.zone.startsWith('Etc/'))).toBe(false);
    expect(zones.filter((choice) => choice.city === 'UTC')).toHaveLength(1);
  });

  it('calls a renamed city by its current name', () => {
    const kyiv = find('Europe/Kyiv') ?? find('Europe/Kiev');
    expect(kyiv?.city).toBe('Kyiv');
    expect(zones.filter((choice) => choice.city === 'Kyiv')).toHaveLength(1);
  });

  it('orders the zones from west to east', () => {
    const minutes = zones.map((choice) => choice.minutes);
    expect(minutes).toEqual([...minutes].sort((a, b) => a - b));
  });

  it('keeps a saved zone that is not in the list', () => {
    expect(timezoneChoices('Etc/GMT-3', winter).some((choice) => choice.zone === 'Etc/GMT-3')).toBe(true);
  });
});

describe('matchesTimezone', () => {
  const winter = new Date(Date.UTC(2026, 0, 15, 12));
  const zones = timezoneChoices('UTC', winter);
  const found = (query: string) =>
    zones.filter((choice) => matchesTimezone(choice, query)).map((choice) => choice.zone);

  it('finds a zone by its city, spaces or underscores alike', () => {
    expect(found('new york')).toContain('America/New_York');
    expect(found('NEW_YORK')).toContain('America/New_York');
    expect(found('tokyo')).toEqual(['Asia/Tokyo']);
  });

  it('finds every zone at an offset, and only those', () => {
    expect(found('gmt+1')).toContain('Europe/Belgrade');
    expect(found('UTC+1')).not.toContain('Pacific/Auckland');
    expect(found('gmt+5:30')).toContain(zones.find((c) => c.city === 'Kolkata')?.zone);
    expect(found('gmt-5')).toContain('America/New_York');
    expect(found('utc')).toContain('UTC');
  });

  it('finds a zone by its region', () => {
    expect(found('argentina')).toContain('America/Argentina/La_Rioja');
  });
});
