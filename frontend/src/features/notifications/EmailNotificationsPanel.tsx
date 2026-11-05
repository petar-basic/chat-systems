import { useTranslation } from 'react-i18next';
import { Mail } from 'lucide-react';
import { toast } from '@/shared/components/Toast';
import { toUserMessage } from '@/lib/errors';
import { useEmailPreference, useSetEmailPreference } from '@/features/notifications/hooks/useEmailPreference';

/**
 * The switch for the digest the worker sends when a mention is still unread. The
 * endpoint shipped without it, which meant people were being emailed with no way
 * to say no from inside the app.
 */
export default function EmailNotificationsPanel() {
  const { t } = useTranslation('notifications');
  const { data: preference } = useEmailPreference();
  const save = useSetEmailPreference();

  // Off at the instance level: no SMTP, nothing to configure.
  if (!preference?.available) return null;

  const toggle = () =>
    save.mutate(!preference.mention_emails, { onError: (e) => toast.error(toUserMessage(e)) });

  return (
    <div className="flex items-start justify-between gap-4" data-qa="email-notifications">
      <div>
        <h4 className="text-sm font-medium text-fg flex items-center gap-2">
          <Mail className="w-4 h-4 text-accent" />
          {t('email.title')}
        </h4>
        <p className="text-xs text-muted mt-1">
          {preference.mention_emails ? t('email.on') : t('email.off')}
        </p>
      </div>
      <button
        type="button"
        onClick={toggle}
        disabled={save.isPending}
        data-qa="email-notifications-toggle"
        className="px-3 py-2 text-sm bg-raised hover:bg-elevated disabled:bg-raised/50 text-fg rounded-lg transition cursor-pointer whitespace-nowrap"
      >
        {preference.mention_emails ? t('turnOff') : t('turnOn')}
      </button>
    </div>
  );
}
