import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';

export type CustomResponse = components['schemas']['CustomResponse'];
export type CustomResponses = components['schemas']['CustomResponses'];
export type NewCustomResponse = components['schemas']['NewCustomResponse'];

export const MAX_TRIGGERS = 20;
export const MAX_ANSWERS = 20;
export const MAX_TRIGGER = 100;
export const MAX_ANSWER = 4000;

export const useCustomResponses = (workspaceId: string, instanceUrl?: string) =>
  useQuery({
    queryKey: QUERY_KEYS.customResponses(workspaceId),
    queryFn: () =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/workspaces/{ws_id}/custom-responses', { params: { path: { ws_id: workspaceId } } }),
      ),
    enabled: !!workspaceId,
    staleTime: 0,
  });

function useRefresh(workspaceId: string) {
  const queryClient = useQueryClient();
  return () =>
    Promise.all([
      queryClient.invalidateQueries({ queryKey: QUERY_KEYS.customResponses(workspaceId) }),
      queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaceMembers(workspaceId) }),
    ]);
}

export const useSaveCustomResponse = (workspaceId: string, instanceUrl?: string) => {
  const refresh = useRefresh(workspaceId);
  return useMutation({
    mutationFn: ({ id, body }: { id: string | null; body: NewCustomResponse }) =>
      getApiForInstance(instanceUrl).typed((c) =>
        id
          ? c.PATCH('/workspaces/{ws_id}/custom-responses/{response_id}', {
              params: { path: { ws_id: workspaceId, response_id: id } },
              body,
            })
          : c.POST('/workspaces/{ws_id}/custom-responses', {
              params: { path: { ws_id: workspaceId } },
              body,
            }),
      ),
    onSettled: refresh,
  });
};

export const useDeleteCustomResponse = (workspaceId: string, instanceUrl?: string) => {
  const refresh = useRefresh(workspaceId);
  return useMutation({
    mutationFn: (id: string) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.DELETE('/workspaces/{ws_id}/custom-responses/{response_id}', {
          params: { path: { ws_id: workspaceId, response_id: id } },
        }),
      ),
    onSettled: refresh,
  });
};

export const useRenameResponsesBot = (workspaceId: string, instanceUrl?: string) => {
  const refresh = useRefresh(workspaceId);
  return useMutation({
    mutationFn: (name: string) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.PUT('/workspaces/{ws_id}/custom-responses/bot', {
          params: { path: { ws_id: workspaceId } },
          body: { name },
        }),
      ),
    onSettled: refresh,
  });
};
