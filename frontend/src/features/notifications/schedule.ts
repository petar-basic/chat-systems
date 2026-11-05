import { formatClock } from '@/lib/datetime';

export const DAY_LABELS = [
  { day: 1, name: 'mon' },
  { day: 2, name: 'tue' },
  { day: 3, name: 'wed' },
  { day: 4, name: 'thu' },
  { day: 5, name: 'fri' },
  { day: 6, name: 'sat' },
  { day: 7, name: 'sun' },
] as const;

export const WEEKDAYS = [1, 2, 3, 4, 5];
export const EVERY_DAY = [1, 2, 3, 4, 5, 6, 7];

export function minuteLabel(minute: number): string {
  const at = new Date(2000, 0, 1, Math.floor(minute / 60), minute % 60);
  return formatClock(at, { hour: 'numeric', minute: '2-digit' });
}

export function timeOptions(current: number): number[] {
  const steps = Array.from({ length: 48 }, (_, i) => i * 30);
  return steps.includes(current) ? steps : [...steps, current].sort((a, b) => a - b);
}
