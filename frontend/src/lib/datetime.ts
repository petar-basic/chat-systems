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

export function formatDay(value: string | Date): string {
  return new Date(value).toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' });
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

export interface TimezoneChoice {
  zone: string;
  city: string;
  region: string;
  offset: string;
  minutes: number;
}

const RENAMED: Record<string, string> = {
  'America/Godthab': 'America/Nuuk',
  'Asia/Calcutta': 'Asia/Kolkata',
  'Asia/Katmandu': 'Asia/Kathmandu',
  'Asia/Rangoon': 'Asia/Yangon',
  'Asia/Saigon': 'Asia/Ho_Chi_Minh',
  'Atlantic/Faeroe': 'Atlantic/Faroe',
  'Europe/Kiev': 'Europe/Kyiv',
  'Pacific/Enderbury': 'Pacific/Kanton',
  'Pacific/Ponape': 'Pacific/Pohnpei',
  'Pacific/Truk': 'Pacific/Chuuk',
};

function offsetOf(zone: string, at: Date): { offset: string; minutes: number } {
  const offset =
    new Intl.DateTimeFormat('en-US', { timeZone: zone, timeZoneName: 'shortOffset' })
      .formatToParts(at)
      .find((part) => part.type === 'timeZoneName')?.value ?? 'GMT';
  const parts = /GMT([+-])(\d{1,2})(?::(\d{2}))?/.exec(offset);
  const minutes = parts ? (parts[1] === '-' ? -1 : 1) * (Number(parts[2]) * 60 + Number(parts[3] ?? 0)) : 0;
  return { offset, minutes };
}

function choiceOf(zone: string, at: Date): TimezoneChoice {
  const segments = (RENAMED[zone] ?? zone).split('/').map((part) => part.replace(/_/g, ' '));
  return {
    zone,
    city: segments[segments.length - 1],
    region: segments.slice(0, -1).join(' · '),
    ...offsetOf(zone, at),
  };
}

export function timezoneChoices(current: string, at: Date = new Date()): TimezoneChoice[] {
  const known = typeof Intl.supportedValuesOf === 'function' ? Intl.supportedValuesOf('timeZone') : [];
  const places = known.filter(
    (zone) =>
      zone.includes('/') && !zone.startsWith('Etc/') && !(RENAMED[zone] && known.includes(RENAMED[zone])),
  );
  const zones = [...new Set(['UTC', ...places, ...(knownZone(current) ? [current] : [])])];
  return zones
    .map((zone) => choiceOf(zone, at))
    .sort((a, b) => a.minutes - b.minutes || a.city.localeCompare(b.city));
}

export function matchesTimezone(choice: TimezoneChoice, query: string): boolean {
  const words = query
    .trim()
    .toLowerCase()
    .replace(/[\s_]+/g, ' ');
  if (!words) return true;
  const offset = /^(?:gmt|utc) ?([+-]) ?(\d{1,2})(?::?(\d{2}))?$/.exec(words);
  if (offset) {
    const minutes = (offset[1] === '-' ? -1 : 1) * (Number(offset[2]) * 60 + Number(offset[3] ?? 0));
    return choice.minutes === minutes;
  }
  if (words === 'gmt' || words === 'utc') return choice.minutes === 0;
  return `${choice.city} ${choice.region} ${choice.zone.replace(/_/g, ' ')}`.toLowerCase().includes(words);
}
