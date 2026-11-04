import { useDisplayStore, type Clock } from '@/stores/display';

const DATE_TIME: Intl.DateTimeFormatOptions = {
  weekday: 'short',
  day: '2-digit',
  month: 'short',
  hour: '2-digit',
  minute: '2-digit',
};

export const TIME: Intl.DateTimeFormatOptions = { hour: '2-digit', minute: '2-digit' };
export const DAY_AND_TIME: Intl.DateTimeFormatOptions = {
  month: 'short',
  day: 'numeric',
  hour: '2-digit',
  minute: '2-digit',
};

export function formatClock(
  value: string | Date,
  options: Intl.DateTimeFormatOptions,
  clock: Clock = useDisplayStore.getState().clock,
): string {
  const date = value instanceof Date ? value : new Date(value);
  if (Number.isNaN(date.getTime())) return '';
  if (clock === 'h12') {
    const hour = options.hour === undefined ? undefined : 'numeric';
    return date.toLocaleString(undefined, { ...options, hour, hourCycle: 'h12' });
  }
  return date.toLocaleString(undefined, { ...options, hourCycle: 'h23' });
}

export function formatDateTime(value: string | Date): string {
  return formatClock(value, DATE_TIME);
}

export function formatTime(value: string | Date): string {
  return formatClock(value, TIME);
}

function knownZone(timezone: string | null | undefined): string | null {
  if (!timezone) return null;
  try {
    new Intl.DateTimeFormat(undefined, { timeZone: timezone });
    return timezone;
  } catch {
    return null;
  }
}

export function localTime(
  timezone: string | null | undefined,
  at: Date = new Date(),
  clock: Clock = useDisplayStore.getState().clock,
): string | null {
  const zone = knownZone(timezone);
  return zone ? formatClock(at, { ...TIME, timeZone: zone }, clock) : null;
}

export type OffHours = 'early' | 'late';

export function offHours(timezone: string | null | undefined, at: Date = new Date()): OffHours | null {
  const zone = knownZone(timezone);
  if (!zone) return null;
  const hour = Number(
    new Intl.DateTimeFormat('en-GB', { hour: 'numeric', hourCycle: 'h23', timeZone: zone }).format(at),
  );
  if (hour < 8) return 'early';
  if (hour >= 21) return 'late';
  return null;
}

export function browserTimezone(): string {
  return Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC';
}

export function timezoneChoices(current: string, search: string): string[] {
  const known = typeof Intl.supportedValuesOf === 'function' ? Intl.supportedValuesOf('timeZone') : [];
  const all = known.includes(current) ? known : [current, ...known];
  const words = search.trim().toLowerCase().replace(/\s+/g, '_');
  const found = words ? all.filter((zone) => zone.toLowerCase().includes(words)) : all;
  return found.includes(current) ? found : [current, ...found];
}
