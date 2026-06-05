import { MESSAGE_GROUP_WINDOW_MS } from '@/shared/constants';
import type { Message } from '@/stores/workspace';

function sameDay(a: Date, b: Date): boolean {
  return a.toDateString() === b.toDateString();
}

export function isNewDay(prev: Message | undefined, curr: Message): boolean {
  if (!prev) return true;
  return !sameDay(new Date(prev.created_at), new Date(curr.created_at));
}

export function isGrouped(prev: Message | undefined, curr: Message): boolean {
  if (!prev) return false;
  if (prev.user_id !== curr.user_id) return false;
  if (curr.failed || prev.failed) return false;
  const prevDate = new Date(prev.created_at);
  const currDate = new Date(curr.created_at);
  if (!sameDay(prevDate, currDate)) return false;
  const dt = currDate.getTime() - prevDate.getTime();
  return dt >= 0 && dt < MESSAGE_GROUP_WINDOW_MS;
}

export function formatDaySeparator(iso: string, now: Date = new Date()): string {
  const d = new Date(iso);
  const yesterday = new Date(now);
  yesterday.setDate(now.getDate() - 1);
  if (sameDay(d, now)) return 'Today';
  if (sameDay(d, yesterday)) return 'Yesterday';
  return d.toLocaleDateString(undefined, {
    weekday: 'long',
    month: 'long',
    day: 'numeric',
    ...(d.getFullYear() !== now.getFullYear() ? { year: 'numeric' } : {}),
  });
}
