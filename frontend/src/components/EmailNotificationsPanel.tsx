import { useEffect, useState } from 'react';
import { Mail } from 'lucide-react';
import { api } from '../lib/api';
import { toast } from '@/shared/components/Toast';
import { toUserMessage } from '@/lib/errors';
import type { components } from '@/api/schema';

type Preference = components['schemas']['EmailPreference'];

/**
 * The switch for the digest the worker sends when a mention reached nobody. The
 * endpoint shipped without it, which meant people were being emailed with no way
 * to say no from inside the app.
 */
export default function EmailNotificationsPanel() {
  const [preference, setPreference] = useState<Preference | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    api
      .typed((c) => c.GET('/notifications/email'))
      .then(setPreference)
      .catch(() => setPreference(null));
  }, []);

  // Off at the instance level: no SMTP, nothing to configure.
  if (!preference?.available) return null;

  const toggle = async () => {
    setBusy(true);
    try {
      const next = !preference.mention_emails;
      await api.typed((c) => c.PATCH('/notifications/email', { body: { mention_emails: next } }));
      setPreference({ ...preference, mention_emails: next });
    } catch (e) {
      toast.error(toUserMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="flex items-start justify-between gap-4" data-qa="email-notifications">
      <div>
        <h4 className="text-sm font-medium text-fg flex items-center gap-2">
          <Mail className="w-4 h-4 text-accent" />
          Email me what I missed
        </h4>
        <p className="text-xs text-muted mt-1">
          {preference.mention_emails
            ? 'On. A mention that reached no device is emailed once, five minutes later — cancelled if you come back first.'
            : 'Off. A mention that reaches no device waits until you open the app.'}
        </p>
      </div>
      <button
        type="button"
        onClick={toggle}
        disabled={busy}
        data-qa="email-notifications-toggle"
        className="px-3 py-2 text-sm bg-raised hover:bg-elevated disabled:bg-raised/50 text-fg rounded-lg transition cursor-pointer whitespace-nowrap"
      >
        {preference.mention_emails ? 'Turn off' : 'Turn on'}
      </button>
    </div>
  );
}
