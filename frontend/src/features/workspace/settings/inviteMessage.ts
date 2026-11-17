import type { components } from '@/api/schema';
import i18n from '@/lib/i18n';

type EmailStatus = components['schemas']['InviteEmailStatus'];

export function inviteMessage(status: EmailStatus | null | undefined, email: string): string {
  switch (status) {
    case 'sent':
      return i18n.t('workspace:settings.invites.sent', { email });
    case 'failed':
      return i18n.t('workspace:settings.invites.emailFailed', { email });
    default:
      return i18n.t('workspace:settings.invites.noEmail', { email });
  }
}
