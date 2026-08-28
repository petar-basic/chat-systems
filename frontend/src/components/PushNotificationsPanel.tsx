import { useEffect, useState } from 'react';
import { Bell, BellOff } from 'lucide-react';
import { currentState, subscribe, unsubscribe, type PushState } from '@/lib/push';
import { toast } from '@/shared/components/Toast';
import { toUserMessage } from '@/lib/errors';

export default function PushNotificationsPanel() {
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
        toast.success('This device will stop receiving notifications.');
      } else if (!(await subscribe())) {
        toast.error('Your browser refused notification permission for this site.');
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
            Notifications on this device
          </h3>
          <p className="text-xs text-muted mt-1">
            {blocked
              ? 'Your browser has blocked notifications for this site. Allow them in the address bar first.'
              : state.subscribed
                ? 'On. Mentions reach you with the app closed; do not disturb and muted channels still apply.'
                : 'Off. Mentions only arrive while a window is open.'}
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
          {state.subscribed ? 'Turn off' : 'Turn on'}
        </button>
      </div>
    </div>
  );
}
