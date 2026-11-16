import { useEffect } from 'react';
import { globalEventBus } from '@/lib/globalEventBus';
import { useDraftStore } from '@/stores/drafts';
import { reconcileDrafts, watchDraftChanges } from '../draftSync';

export function useDraftSync(workspaceId?: string, instanceUrl?: string) {
  useEffect(() => watchDraftChanges(), []);

  useEffect(
    () =>
      globalEventBus.on('draft.updated', (event, from) => {
        const url = from ?? instanceUrl;
        if (url) useDraftStore.getState().accept(url, event.draft);
      }),
    [instanceUrl],
  );

  useEffect(() => {
    if (!workspaceId || !instanceUrl) return undefined;
    const run = () => {
      reconcileDrafts(instanceUrl, workspaceId).catch(() => undefined);
    };
    run();
    const offSync = globalEventBus.on('sync.complete', (event) => {
      if (event.workspace_id === workspaceId) run();
    });
    window.addEventListener('online', run);
    return () => {
      offSync();
      window.removeEventListener('online', run);
    };
  }, [workspaceId, instanceUrl]);
}
