import { formatClock } from './datetime';

export interface DndState {
  dnd_until?: string | null;
  schedule_paused_until?: string | null;
}

export interface Pause {
  until: string;
  bySchedule: boolean;
}

export const PAUSE_PRESETS = [
  { name: 'halfHour', minutes: 30 },
  { name: 'hour', minutes: 60 },
  { name: 'twoHours', minutes: 120 },
] as const;

export function minutesFromNowIso(minutes: number, now = Date.now()): string {
  return new Date(now + minutes * 60000).toISOString();
}

export function tomorrowMorningIso(now = new Date()): string {
  const d = new Date(now);
  d.setDate(d.getDate() + 1);
  d.setHours(8, 0, 0, 0);
  return d.toISOString();
}

export function currentPause(state: DndState | null | undefined, now = Date.now()): Pause | null {
  const live = (iso: string | null | undefined) => (iso && Date.parse(iso) > now ? iso : null);
  const timer = live(state?.dnd_until);
  const schedule = live(state?.schedule_paused_until);
  if (timer && (!schedule || Date.parse(timer) >= Date.parse(schedule))) {
    return { until: timer, bySchedule: false };
  }
  return schedule ? { until: schedule, bySchedule: true } : null;
}

export function formatResume(iso: string, now = new Date()): string {
  const at = new Date(iso);
  const time = formatClock(at, { hour: 'numeric', minute: '2-digit' });
  if (at.toDateString() === now.toDateString()) return time;
  return `${at.toLocaleDateString([], { weekday: 'short' })} ${time}`;
}
