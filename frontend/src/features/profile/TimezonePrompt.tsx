import { useEffect, useEffectEvent, useState } from 'react';
import { ConfirmDialog } from '@/shared/components/ConfirmDialog/ConfirmDialog';
import { useWorkspaceStore } from '@/stores/workspace';
import { browserTimezone } from '@/lib/datetime';
import { useAccount } from './hooks/useProfile';
import { useUpdateTimezone } from './hooks/useTimezone';
import { askedAbout, rememberAsked } from './timezoneQuestion';

export function TimezonePrompt() {
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
      title="Update your time zone?"
      description={`This device is set to ${browser}, but your profile says ${saved}. The local time people see on your profile and your notification schedule follow your profile.`}
      confirmLabel={`Use ${browser}`}
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
