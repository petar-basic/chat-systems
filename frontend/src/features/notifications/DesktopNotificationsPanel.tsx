import { useState } from 'react';
import { AppWindow } from 'lucide-react';
import { Button } from '@/shared/components/Button/Button';
import { useNotificationPrefs } from '@/stores/notificationPrefs';
import { requestNotificationPermission } from '@/lib/notifications';

export default function DesktopNotificationsPanel() {
  const { desktopEnabled, toggleDesktop } = useNotificationPrefs();
  const [permission, setPermission] = useState(() =>
    'Notification' in window ? Notification.permission : null,
  );

  if (!permission) return null;

  const blocked = permission === 'denied';
  const on = desktopEnabled && permission === 'granted';

  const toggle = async () => {
    if (on) {
      toggleDesktop();
      return;
    }
    const granted = await requestNotificationPermission();
    setPermission(Notification.permission);
    if (granted && !desktopEnabled) toggleDesktop();
  };

  return (
    <div className="flex items-start justify-between gap-4" data-qa="desktop-notifications">
      <div>
        <h3 className="text-sm font-medium text-fg flex items-center gap-2">
          <AppWindow className="w-4 h-4 text-accent" />
          Desktop notifications while the app is open
        </h3>
        <p className="text-xs text-muted mt-1">
          {blocked
            ? 'Your browser has blocked notifications for this site. Allow them in the address bar first.'
            : on
              ? 'On. A message that notifies you shows on the desktop while this window is not in front.'
              : 'Off. An open window only plays a sound and updates Activity.'}
        </p>
      </div>
      <Button onClick={() => void toggle()} disabled={blocked} data-qa="desktop-notifications-toggle">
        {on ? 'Turn off' : 'Turn on'}
      </Button>
    </div>
  );
}
