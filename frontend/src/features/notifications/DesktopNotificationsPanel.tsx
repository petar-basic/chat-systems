import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { AppWindow } from 'lucide-react';
import { Button } from '@/shared/components/Button/Button';
import { useNotificationPrefs } from '@/stores/notificationPrefs';
import { notificationPermission, requestNotificationPermission } from '@/lib/notifications';

export default function DesktopNotificationsPanel() {
  const { t } = useTranslation('notifications');
  const { desktopEnabled, toggleDesktop } = useNotificationPrefs();
  const [permission, setPermission] = useState(notificationPermission);

  if (!permission) return null;

  const blocked = permission === 'denied';
  const on = desktopEnabled && permission === 'granted';

  const toggle = async () => {
    if (on) {
      toggleDesktop();
      return;
    }
    const granted = await requestNotificationPermission();
    setPermission(notificationPermission());
    if (granted && !desktopEnabled) toggleDesktop();
  };

  return (
    <div className="flex items-start justify-between gap-4" data-qa="desktop-notifications">
      <div>
        <h3 className="text-sm font-medium text-fg flex items-center gap-2">
          <AppWindow className="w-4 h-4 text-accent" />
          {t('desktop.title')}
        </h3>
        <p className="text-xs text-muted mt-1">
          {blocked ? t('blocked') : on ? t('desktop.on') : t('desktop.off')}
        </p>
      </div>
      <Button onClick={() => void toggle()} disabled={blocked} data-qa="desktop-notifications-toggle">
        {on ? t('turnOff') : t('turnOn')}
      </Button>
    </div>
  );
}
