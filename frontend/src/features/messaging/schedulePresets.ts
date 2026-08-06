export interface SchedulePreset {
  label: string;
  at: () => Date;
}

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

export const SCHEDULE_PRESETS: SchedulePreset[] = [
  { label: 'In an hour', at: () => new Date(Date.now() + 60 * 60 * 1000) },
  { label: 'Tomorrow at 9:00', at: () => nextMorning(new Date(), 1) },
  { label: 'Monday at 9:00', at: () => nextMondayMorning(new Date()) },
];

export function toLocalInputValue(at: Date): string {
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${at.getFullYear()}-${pad(at.getMonth() + 1)}-${pad(at.getDate())}T${pad(at.getHours())}:${pad(at.getMinutes())}`;
}

export function formatScheduleHint(at: Date, now: Date = new Date()): string {
  const time = at.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  if (at.toDateString() === now.toDateString()) return time;
  return `${at.toLocaleDateString([], { weekday: 'short', day: 'numeric', month: 'short' })}, ${time}`;
}
