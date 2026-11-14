import type { InviteRequest, RequestTerms } from '@/features/workspace/hooks/useInviteRequests';
import { dayOf, endOfDay, type GuestInvite } from './guestTerms';

export type RequestRole = 'member' | 'guest';

export const MAX_EMAILS = 20;
export const MAX_NOTE = 1000;

export function parseEmails(text: string): string[] {
  const seen = new Set<string>();
  for (const part of text.split(/[\s,;]+/)) {
    const email = part.trim().toLowerCase();
    if (email) seen.add(email);
  }
  return [...seen];
}

export function termsFor(emails: string[], role: RequestRole, guest: GuestInvite): RequestTerms {
  return role === 'guest'
    ? {
        emails,
        role,
        guest_kind: guest.kind,
        channel_ids: guest.channelIds,
        guest_until: guest.until ? endOfDay(guest.until) : null,
      }
    : { emails, role, channel_ids: [] };
}

export function guestOf(request: InviteRequest): GuestInvite {
  return {
    kind: request.guest_kind ?? 'multi',
    channelIds: request.channel_ids,
    until: request.guest_until ? dayOf(request.guest_until) : '',
  };
}

export function roleOf(request: InviteRequest): RequestRole {
  return request.role === 'guest' ? 'guest' : 'member';
}

export function readyToSend(emails: string[], role: RequestRole, guest: GuestInvite): boolean {
  return (
    emails.length > 0 && emails.length <= MAX_EMAILS && (role !== 'guest' || guest.channelIds.length > 0)
  );
}
