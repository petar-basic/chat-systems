import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { useCurrentApi } from '@/shared/hooks/useCurrentApi';
import { useWorkspaceStore } from '@/stores/workspace';
import { QUERY_KEYS, ErrorLabels } from '@/shared/constants';
import { toast } from '@/shared/components/Toast';
import { toUserMessage } from '@/lib/errors';
import { logger } from '@/lib/logger';
import { rememberPendingConnect } from '@/lib/appConnect';

export type AppStatus = components['schemas']['AppStatus'];
export type AppProvider = components['schemas']['Provider'];

export function useApps(workspaceId: string | undefined) {
  const apiClient = useCurrentApi();
  return useQuery({
    queryKey: QUERY_KEYS.apps(workspaceId ?? ''),
    enabled: !!workspaceId,
    queryFn: () =>
      apiClient.typed((c) =>
        c.GET('/workspaces/{ws_id}/apps', { params: { path: { ws_id: workspaceId ?? '' } } }),
      ),
  });
}

export function useAppActions(workspaceId: string | undefined) {
  const apiClient = useCurrentApi();
  const queryClient = useQueryClient();
  const ws = workspaceId ?? '';
  const refetch = () => queryClient.invalidateQueries({ queryKey: QUERY_KEYS.apps(ws) });
  const failed = (where: string, label: string) => (err: unknown) => {
    logger.error('useAppActions', where, err);
    toast.error(toUserMessage(err, label));
  };

  const configure = useMutation({
    mutationFn: (vars: { provider: AppProvider; clientId: string; clientSecret: string; enabled: boolean }) =>
      apiClient.typed((c) =>
        c.PUT('/workspaces/{ws_id}/apps/{provider}', {
          params: { path: { ws_id: ws, provider: vars.provider } },
          body: {
            client_id: vars.clientId,
            client_secret: vars.clientSecret || null,
            enabled: vars.enabled,
          },
        }),
      ),
    onSuccess: refetch,
    onError: failed('configure', ErrorLabels.AppSaveFailed),
  });

  const remove = useMutation({
    mutationFn: (provider: AppProvider) =>
      apiClient.typed((c) =>
        c.DELETE('/workspaces/{ws_id}/apps/{provider}', { params: { path: { ws_id: ws, provider } } }),
      ),
    onSuccess: refetch,
    onError: failed('remove', ErrorLabels.AppSaveFailed),
  });

  const connect = useMutation({
    mutationFn: async ({
      provider,
      refreshMessageId,
    }: {
      provider: AppProvider;
      refreshMessageId?: string;
    }) => {
      const response = await apiClient.typed((c) =>
        c.POST('/workspaces/{ws_id}/apps/{provider}/connect', {
          params: { path: { ws_id: ws, provider } },
          body: { return_to: window.location.pathname },
        }),
      );
      rememberPendingConnect({
        instanceUrl: useWorkspaceStore.getState().currentWorkspace?.instanceUrl,
        refreshMessageId,
      });
      window.location.assign(response.authorize_url);
    },
    onError: failed('connect', ErrorLabels.AppConnectFailed),
  });

  const disconnect = useMutation({
    mutationFn: (provider: AppProvider) =>
      apiClient.typed((c) =>
        c.DELETE('/workspaces/{ws_id}/apps/{provider}/connection', {
          params: { path: { ws_id: ws, provider } },
        }),
      ),
    onSuccess: refetch,
    onError: failed('disconnect', ErrorLabels.AppSaveFailed),
  });

  const prompts = useMutation({
    mutationFn: ({ provider, off }: { provider: AppProvider; off: boolean }) => {
      const params = { params: { path: { ws_id: ws, provider } } };
      return apiClient.typed((c) =>
        off
          ? c.PUT('/workspaces/{ws_id}/apps/{provider}/prompts', params)
          : c.DELETE('/workspaces/{ws_id}/apps/{provider}/prompts', params),
      );
    },
    onSuccess: refetch,
    onError: failed('prompts', ErrorLabels.AppSaveFailed),
  });

  return { configure, remove, connect, disconnect, prompts };
}
