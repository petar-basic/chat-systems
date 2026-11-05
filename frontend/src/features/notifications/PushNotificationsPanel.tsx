import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Bell, BellOff } from 'lucide-react';
import { currentState, subscribe, unsubscribe, type PushState } from '@/lib/push';
import { toast } from '@/shared/components/Toast';
import { toUserMessage } from '@/lib/errors';

export default function PushNotificationsPanel() {
  const { t } = useTranslation('notifications');
  const [state, setState] = useState<PushState | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    void currentState().then(setState);
  }, []);

  if (!state || !state.supported || !state.enabled) return null;

  const toggle = async () => {
    setBusy(true);
    try {
      if (state.subscribed) {
        await unsubscribe();
        toast.success(t('push.stopped'));
      } else if (!(await subscribe())) {
        toast.error(t('push.refused'));
      }
      setState(await currentState());
    } catch (e) {
      toast.error(toUserMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const blocked = state.permission === 'denied' && !state.subscribed;

  return (
    <div data-qa="push-notifications">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h3 className="text-sm font-medium text-fg flex items-center gap-2">
            <Bell className="w-4 h-4 text-accent" />
            {t('push.title')}
          </h3>
          <p className="text-xs text-muted mt-1">
            {blocked ? t('blocked') : state.subscribed ? t('push.on') : t('push.off')}
          </p>
        </div>
        <button
          type="button"
          onClick={toggle}
          disabled={busy || blocked}
          data-qa="push-toggle"
          className="flex items-center gap-1.5 px-3 py-2 text-sm bg-raised hover:bg-elevated disabled:bg-raised/50 disabled:cursor-not-allowed text-fg rounded-lg transition cursor-pointer whitespace-nowrap"
        >
          {state.subscribed ? <BellOff className="w-4 h-4" /> : <Bell className="w-4 h-4" />}
          {state.subscribed ? t('turnOff') : t('turnOn')}
        </button>
      </div>
    </div>
  );
}
