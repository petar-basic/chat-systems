import { useState } from 'react';
import { LayoutGrid, X } from 'lucide-react';
import { useWorkspaceStore } from '@/stores/workspace';
import { useAppActions, type AppProvider } from '@/hooks/queries/useApps';
import { appName } from '@/lib/appConnect';

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
  const [dismissed, setDismissed] = useState(() => wasDismissed(messageId));
  if (dismissed || providers.length === 0) return null;

  const dismiss = () => {
    setDismissed(true);
    try {
      localStorage.setItem(dismissKey(messageId), '1');
    } catch {
      return;
    }
  };

  return (
    <div className="mt-1 flex flex-wrap items-center gap-2 text-xs text-muted" data-qa="app-connect-prompt">
      <LayoutGrid className="h-3.5 w-3.5" />
      <span>Only you can see this.</span>
      {providers.map((provider) => (
        <button
          key={provider}
          type="button"
          onClick={() =>
            actions.connect.mutate({ provider: provider as AppProvider, refreshMessageId: messageId })
          }
          disabled={actions.connect.isPending}
          className="text-accent hover:underline disabled:opacity-50"
        >
          Connect {appName(provider)} to show a preview
        </button>
      ))}
      <button type="button" onClick={dismiss} aria-label="Dismiss" className="rounded p-0.5 hover:bg-raised">
        <X className="h-3 w-3" />
      </button>
    </div>
  );
}
