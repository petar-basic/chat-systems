import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { useCurrentApi } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';

export type BotApp = components['schemas']['BotApp'];
export type BotAppWithToken = components['schemas']['BotAppWithToken'];
export type BotScope = components['schemas']['Scope'];

export interface NewBotApp {
  name: string;
  description: string;
  scopes: BotScope[];
}

export function useBotApps(workspaceId: string | undefined) {
  const apiClient = useCurrentApi();
  return useQuery({
    queryKey: QUERY_KEYS.botApps(workspaceId ?? ''),
    enabled: !!workspaceId,
    queryFn: () =>
      apiClient.typed((c) =>
        c.GET('/workspaces/{ws_id}/bot-apps', { params: { path: { ws_id: workspaceId ?? '' } } }),
      ),
  });
}

export function useBotAppActions(workspaceId: string | undefined) {
  const apiClient = useCurrentApi();
  const queryClient = useQueryClient();
  const ws = workspaceId ?? '';
  const refetch = () =>
    Promise.all([
      queryClient.invalidateQueries({ queryKey: QUERY_KEYS.botApps(ws) }),
      queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaceMembers(ws) }),
    ]);

  const create = useMutation({
    mutationFn: (app: NewBotApp) =>
      apiClient.typed((c) =>
        c.POST('/workspaces/{ws_id}/bot-apps', {
          params: { path: { ws_id: ws } },
          body: { name: app.name, description: app.description || null, scopes: app.scopes },
        }),
      ),
    onSuccess: refetch,
  });

  const rotate = useMutation({
    mutationFn: (appId: string) =>
      apiClient.typed((c) => c.POST('/bot-apps/{app_id}/rotate', { params: { path: { app_id: appId } } })),
  });

  const remove = useMutation({
    mutationFn: (appId: string) =>
      apiClient.typed((c) => c.DELETE('/bot-apps/{app_id}', { params: { path: { app_id: appId } } })),
    onSuccess: refetch,
  });

  return { create, rotate, remove };
}
