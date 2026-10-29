import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { useCurrentApi } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';

export type BotApp = components['schemas']['BotApp'];
export type BotAppWithToken = components['schemas']['BotAppWithToken'];
export type BotScope = components['schemas']['Scope'];
export type AppEvent = components['schemas']['AppEvent'];

export interface EventSettings {
  events: AppEvent[];
  request_url: string;
  socket_mode: boolean;
}

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
      queryClient.invalidateQueries({ queryKey: QUERY_KEYS.appDirectory(ws) }),
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

  const configure = useMutation({
    mutationFn: ({ appId, settings }: { appId: string; settings: EventSettings }) =>
      apiClient.typed((c) =>
        c.PATCH('/bot-apps/{app_id}', {
          params: { path: { app_id: appId } },
          body: settings,
        }),
      ),
    onSuccess: refetch,
  });

  const setScopes = useMutation({
    mutationFn: ({ appId, scopes }: { appId: string; scopes: BotScope[] }) =>
      apiClient.typed((c) =>
        c.PATCH('/bot-apps/{app_id}', { params: { path: { app_id: appId } }, body: { scopes } }),
      ),
    onSuccess: refetch,
  });

  const makeAppToken = useMutation({
    mutationFn: (appId: string) =>
      apiClient.typed((c) => c.POST('/bot-apps/{app_id}/app-token', { params: { path: { app_id: appId } } })),
    onSuccess: refetch,
  });

  const revokeAppToken = useMutation({
    mutationFn: (appId: string) =>
      apiClient.typed((c) =>
        c.DELETE('/bot-apps/{app_id}/app-token', { params: { path: { app_id: appId } } }),
      ),
    onSuccess: refetch,
  });

  const revealSecret = useMutation({
    mutationFn: (appId: string) =>
      apiClient.typed((c) =>
        c.GET('/bot-apps/{app_id}/signing-secret', { params: { path: { app_id: appId } } }),
      ),
  });

  return { create, rotate, remove, configure, setScopes, makeAppToken, revokeAppToken, revealSecret };
}
