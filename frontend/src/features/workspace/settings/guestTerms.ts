import type { components } from '@/api/schema';
import type { Channel } from '@/stores/workspace';

export type GuestKind = components['schemas']['GuestKind'];
type BrowsableChannel = components['schemas']['BrowsableChannel'];

export interface GuestInvite {
  kind: GuestKind;
  channelIds: string[];
  until: string;
}

export interface GuestChoice {
  id: string;
  name: string;
}

const ENDING_SOON_DAYS = 30;

export function guestChannelChoices(mine: Channel[], browsable: BrowsableChannel[]): GuestChoice[] {
  const byId = new Map<string, GuestChoice>();
  for (const ch of mine) {
    if (ch.is_archived || (ch.channel_type !== 'public' && ch.channel_type !== 'private') || !ch.name)
      continue;
    byId.set(ch.id, { id: ch.id, name: ch.name });
  }
  for (const ch of browsable) {
    if (ch.is_archived || ch.channel_type !== 'public' || !ch.name) continue;
    byId.set(ch.id, { id: ch.id, name: ch.name });
  }
  return [...byId.values()].sort((a, b) => a.name.localeCompare(b.name));
}

export function endOfDay(day: string): string {
  return new Date(`${day}T23:59:59`).toISOString();
}

export function dayOf(value: string): string {
  const date = new Date(value);
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

export function tomorrow(now: Date = new Date()): string {
  const next = new Date(now);
  next.setDate(next.getDate() + 1);
  return dayOf(next.toISOString());
}

export function toggleChannel(invite: GuestInvite, id: string, on: boolean): GuestInvite {
  if (invite.kind === 'single') return { ...invite, channelIds: on ? [id] : [] };
  const rest = invite.channelIds.filter((x) => x !== id);
  return { ...invite, channelIds: on ? [...rest, id] : rest };
}

export function endingSoon(until: string | null | undefined, now: Date = new Date()): boolean {
  if (!until) return false;
  const left = new Date(until).getTime() - now.getTime();
  return left > 0 && left <= ENDING_SOON_DAYS * 24 * 60 * 60 * 1000;
}
