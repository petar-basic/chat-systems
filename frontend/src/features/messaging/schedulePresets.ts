import { formatTime } from '@/lib/datetime';

export function nextMorning(from: Date, addDays: number): Date {
  const at = new Date(from);
  at.setDate(at.getDate() + addDays);
  at.setHours(9, 0, 0, 0);
  return at;
}

export function nextMondayMorning(from: Date): Date {
  const daysUntilMonday = (8 - from.getDay()) % 7 || 7;
  return nextMorning(from, daysUntilMonday);
}

export const SCHEDULE_PRESETS = [
  { name: 'inAnHour', at: () => new Date(Date.now() + 60 * 60 * 1000) },
  { name: 'tomorrow', at: () => nextMorning(new Date(), 1) },
  { name: 'monday', at: () => nextMondayMorning(new Date()) },
] as const;

export const REMIND_PRESETS = [
  { name: 'in20Minutes', at: () => new Date(Date.now() + 20 * 60 * 1000) },
  { name: 'in1Hour', at: () => new Date(Date.now() + 60 * 60 * 1000) },
  { name: 'in3Hours', at: () => new Date(Date.now() + 3 * 60 * 60 * 1000) },
  { name: 'tomorrow', at: () => nextMorning(new Date(), 1) },
  { name: 'nextWeek', at: () => nextMondayMorning(new Date()) },
] as const;

export function toLocalInputValue(at: Date): string {
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${at.getFullYear()}-${pad(at.getMonth() + 1)}-${pad(at.getDate())}T${pad(at.getHours())}:${pad(at.getMinutes())}`;
}

export function formatScheduleHint(at: Date, now: Date = new Date()): string {
  const time = formatTime(at);
  if (at.toDateString() === now.toDateString()) return time;
  return `${at.toLocaleDateString([], { weekday: 'short', day: 'numeric', month: 'short' })}, ${time}`;
}
