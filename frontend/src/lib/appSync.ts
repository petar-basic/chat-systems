import type { QueryClient } from '@tanstack/react-query';
import { globalEventBus } from './globalEventBus';
import { QUERY_KEYS } from '@/shared/constants';

export function syncAppFrames(queryClient: QueryClient): Array<() => void> {
  return [
    globalEventBus.on('workspace.apps_changed', (event) => {
      queryClient.invalidateQueries({ queryKey: QUERY_KEYS.botApps(event.workspace_id) });
      queryClient.invalidateQueries({ queryKey: QUERY_KEYS.appHomeAll() });
      queryClient.invalidateQueries({
        predicate: ({ queryKey }) => queryKey[0] === 'channels' && queryKey[2] === 'message-shortcuts',
      });
    }),
    globalEventBus.on('views.home_changed', (event) => {
      queryClient.setQueryData(QUERY_KEYS.appHome(event.app_id), { app_id: event.app_id, home: event.home });
    }),
  ];
}
