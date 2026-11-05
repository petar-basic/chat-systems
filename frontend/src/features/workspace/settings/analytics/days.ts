export function dayLabel(day: string | null | undefined): string {
  if (!day) return '—';
  const date = new Date(`${day}T00:00:00Z`);
  if (Number.isNaN(date.getTime())) return day;
  return date.toLocaleDateString(undefined, { month: 'short', day: 'numeric', timeZone: 'UTC' });
}

export function lastMessageDay(at: string | null | undefined): string | null {
  return at ? at.slice(0, 10) : null;
}
