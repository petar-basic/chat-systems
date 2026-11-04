import { useEffect, useEffectEvent, useState } from 'react';
import { ConfirmDialog } from '@/shared/components/ConfirmDialog/ConfirmDialog';
import { useWorkspaceStore } from '@/stores/workspace';
import { browserTimezone } from '@/lib/datetime';
import { useAccount } from './hooks/useProfile';
import { useUpdateTimezone } from './hooks/useTimezone';

const ASKED_KEY = 'chat_timezone_asked';

function askedAbout(): string | null {
  try {
    return localStorage.getItem(ASKED_KEY);
  } catch {
    return null;
  }
}

function rememberAsked(zone: string) {
  try {
    localStorage.setItem(ASKED_KEY, zone);
  } catch {
    return;
  }
}

export function TimezonePrompt() {
  const instanceUrl = useWorkspaceStore((s) => s.currentWorkspace?.instanceUrl);
  const { data: account } = useAccount(instanceUrl);
  const update = useUpdateTimezone();
  const browser = browserTimezone();
  const [answered, setAnswered] = useState(() => askedAbout() === browser);

  const saved = account?.timezone;
  const unset = account !== undefined && saved == null;
  const fillIn = useEffectEvent(() => update.mutate(browser));
  useEffect(() => {
    if (unset) fillIn();
  }, [unset]);

  if (!account || saved == null || saved === browser || answered) return null;

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
