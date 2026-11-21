import { useEffect, useEffectEvent, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ConfirmDialog } from '@/shared/components/ConfirmDialog/ConfirmDialog';
import { useWorkspaceStore } from '@/stores/workspace';
import { browserTimezone } from '@/lib/datetime';
import { useAccount } from './hooks/useProfile';
import { useUpdateTimezone } from './hooks/useTimezone';
import { askedAbout, rememberAsked } from './timezoneQuestion';

export function TimezonePrompt() {
  const { t } = useTranslation('profile');
  const instanceUrl = useWorkspaceStore((s) => s.currentWorkspace?.instanceUrl);
  const { data: account } = useAccount(instanceUrl);
  const update = useUpdateTimezone();
  const browser = browserTimezone();
  const [, setAnswered] = useState(false);

  const saved = account?.timezone;
  const unset = account !== undefined && saved == null;
  const fillIn = useEffectEvent(() => update.mutate(browser));
  useEffect(() => {
    if (unset) fillIn();
  }, [unset]);

  if (!account || saved == null || saved === browser || askedAbout() === browser) return null;

  const close = () => {
    rememberAsked(browser);
    setAnswered(true);
  };

  return (
    <ConfirmDialog
      title={t('region.prompt.title')}
      description={t('region.prompt.body', { device: browser, profile: saved })}
      confirmLabel={t('region.prompt.use', { device: browser })}
      tone="primary"
      onConfirm={() => {
        update.mutate(browser);
        close();
      }}
      onClose={close}
      dataQa="timezone-prompt"
    />
  );
}
