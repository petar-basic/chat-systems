export const DAY_LABELS = [
  { day: 1, short: 'Mon', long: 'Monday' },
  { day: 2, short: 'Tue', long: 'Tuesday' },
  { day: 3, short: 'Wed', long: 'Wednesday' },
  { day: 4, short: 'Thu', long: 'Thursday' },
  { day: 5, short: 'Fri', long: 'Friday' },
  { day: 6, short: 'Sat', long: 'Saturday' },
  { day: 7, short: 'Sun', long: 'Sunday' },
];

export const WEEKDAYS = [1, 2, 3, 4, 5];
export const EVERY_DAY = [1, 2, 3, 4, 5, 6, 7];

export function minuteLabel(minute: number): string {
  const at = new Date(2000, 0, 1, Math.floor(minute / 60), minute % 60);
  return at.toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' });
}

export function timeOptions(current: number): number[] {
  const steps = Array.from({ length: 48 }, (_, i) => i * 30);
  return steps.includes(current) ? steps : [...steps, current].sort((a, b) => a - b);
}
