import type { components } from '@/api/schema';

type EmailStatus = components['schemas']['InviteEmailStatus'];

export function inviteMessage(status: EmailStatus | null | undefined, email: string): string {
  switch (status) {
    case 'sent':
      return `Invite sent to ${email}. You can also send them the link yourself.`;
    case 'failed':
      return `The invite email to ${email} could not be sent. Send them this link yourself.`;
    default:
      return `Email is not set up on this instance, so nothing was sent. Send ${email} this link yourself.`;
  }
}
