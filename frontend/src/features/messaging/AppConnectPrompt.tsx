import { useState } from 'react';
import { Button } from '@/shared/components/Button/Button';
import { useWorkspaceStore } from '@/stores/workspace';
import { useAppActions, useApps, type AppProvider } from '@/features/workspace';
import { appName } from '@/lib/appConnect';
import { AppIcon } from '@/shared/components/AppIcon/AppIcon';

const dismissKey = (messageId: string) => `app-prompt-dismissed:${messageId}`;

function wasDismissed(messageId: string): boolean {
  try {
    return localStorage.getItem(dismissKey(messageId)) === '1';
  } catch {
    return false;
  }
}

export function AppConnectPrompt({ providers, messageId }: { providers: string[]; messageId: string }) {
  const workspaceId = useWorkspaceStore((s) => s.currentWorkspace?.id);
  const actions = useAppActions(workspaceId);
  const { data } = useApps(workspaceId);
  const [dismissed, setDismissed] = useState(() => wasDismissed(messageId));

  const settled = new Set(
    (data?.data ?? [])
      .filter((app) => app.prompts_off || app.connected_account)
      .map((app) => app.provider as string),
  );
  const asking = providers.filter((provider) => !settled.has(provider));
  if (dismissed || asking.length === 0) return null;

  const notNow = () => {
    setDismissed(true);
    try {
      localStorage.setItem(dismissKey(messageId), '1');
    } catch {
      return;
    }
  };

  return (
    <div className="mt-1 flex max-w-md flex-col gap-2" data-qa="app-connect-prompt">
      {asking.map((provider) => (
        <div key={provider} className="rounded-lg border border-line/70 bg-surface/40 px-3 py-2">
          <div className="flex items-center gap-1.5 text-xs text-muted">
            <AppIcon provider={provider} />
            <span className="font-medium text-fg-dim">{appName(provider)}</span>
            <span>· Only visible to you</span>
          </div>
          <p className="mt-1 text-sm text-fg-soft">
            Connect your {appName(provider)} account to show a preview of this link.
          </p>
          <div className="mt-2 flex flex-wrap gap-2">
            <Button
              variant="primary"
              size="sm"
              onClick={() =>
                actions.connect.mutate({ provider: provider as AppProvider, refreshMessageId: messageId })
              }
              disabled={actions.connect.isPending}
            >
              Connect {appName(provider)}
            </Button>
            <Button size="sm" onClick={notNow}>
              Not now
            </Button>
            <Button
              size="sm"
              onClick={() => actions.prompts.mutate({ provider: provider as AppProvider, off: true })}
              disabled={actions.prompts.isPending}
            >
              Never ask me again
            </Button>
          </div>
        </div>
      ))}
    </div>
  );
}
