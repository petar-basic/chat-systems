import { keepPreviousData, useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';

export type ProfileField = components['schemas']['ProfileField'];
export type ProfileFieldKind = components['schemas']['ProfileFieldKind'];
export type ProfileFieldOption = components['schemas']['ProfileFieldOption'];
export type NewProfileField = components['schemas']['NewProfileField'];
export type UpdateProfileField = components['schemas']['UpdateProfileField'];

const NO_FIELDS: ProfileField[] = [];

export const useProfileFields = (workspaceId: string | undefined, instanceUrl?: string) => {
  const query = useQuery({
    queryKey: QUERY_KEYS.profileFields(workspaceId ?? ''),
    queryFn: async () => {
      const response = await getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/workspaces/{ws_id}/profile-fields', { params: { path: { ws_id: workspaceId ?? '' } } }),
      );
      return response.data;
    },
    enabled: !!workspaceId,
    staleTime: 1000 * 60 * 5,
  });
  return { ...query, fields: query.data ?? NO_FIELDS };
};

export const useDirectory = (
  workspaceId: string,
  q: string,
  filter: string,
  instanceUrl?: string,
  enabled = true,
) =>
  useQuery({
    queryKey: QUERY_KEYS.directory(workspaceId, q, filter),
    queryFn: async () => {
      const response = await getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/workspaces/{ws_id}/members', {
          params: {
            path: { ws_id: workspaceId },
            query: { q: q || undefined, filter: filter || undefined },
          },
        }),
      );
      return response.data;
    },
    enabled: !!workspaceId && enabled,
    placeholderData: keepPreviousData,
    staleTime: 1000 * 30,
  });

function useRefresh(workspaceId: string) {
  const queryClient = useQueryClient();
  return () => {
    void queryClient.invalidateQueries({ queryKey: QUERY_KEYS.profileFields(workspaceId) });
    void queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaceMembers(workspaceId) });
  };
}

export const useCreateProfileField = (workspaceId: string, instanceUrl?: string) => {
  const refresh = useRefresh(workspaceId);
  return useMutation({
    mutationFn: (body: NewProfileField) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.POST('/workspaces/{ws_id}/profile-fields', { params: { path: { ws_id: workspaceId } }, body }),
      ),
    onSuccess: refresh,
  });
};

export const useUpdateProfileField = (workspaceId: string, instanceUrl?: string) => {
  const refresh = useRefresh(workspaceId);
  return useMutation({
    mutationFn: ({ fieldId, body }: { fieldId: string; body: UpdateProfileField }) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.PATCH('/workspaces/{ws_id}/profile-fields/{field_id}', {
          params: { path: { ws_id: workspaceId, field_id: fieldId } },
          body,
        }),
      ),
    onSuccess: refresh,
  });
};

export const useDeleteProfileField = (workspaceId: string, instanceUrl?: string) => {
  const refresh = useRefresh(workspaceId);
  return useMutation({
    mutationFn: (fieldId: string) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.DELETE('/workspaces/{ws_id}/profile-fields/{field_id}', {
          params: { path: { ws_id: workspaceId, field_id: fieldId } },
        }),
      ),
    onSuccess: refresh,
  });
};

export const useOrderProfileFields = (workspaceId: string, instanceUrl?: string) => {
  const queryClient = useQueryClient();
  const key = QUERY_KEYS.profileFields(workspaceId);
  return useMutation({
    mutationFn: (fieldIds: string[]) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.PUT('/workspaces/{ws_id}/profile-fields/order', {
          params: { path: { ws_id: workspaceId } },
          body: { field_ids: fieldIds },
        }),
      ),
    onMutate: async (fieldIds) => {
      await queryClient.cancelQueries({ queryKey: key });
      const before = queryClient.getQueryData<ProfileField[]>(key);
      if (before) {
        const byId = new Map(before.map((f) => [f.id, f]));
        queryClient.setQueryData(
          key,
          fieldIds.flatMap((id) => byId.get(id) ?? []),
        );
      }
      return { before };
    },
    onError: (_err, _ids, context) => {
      if (context?.before) queryClient.setQueryData(key, context.before);
    },
    onSettled: () => void queryClient.invalidateQueries({ queryKey: key }),
  });
};

export const useSetProfileValues = (workspaceId: string, instanceUrl?: string) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ userId, values }: { userId: string; values: Record<string, string | null> }) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.PUT('/workspaces/{ws_id}/members/{user_id}/profile-fields', {
          params: { path: { ws_id: workspaceId, user_id: userId } },
          body: { values },
        }),
      ),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaceMembers(workspaceId) });
      void queryClient.invalidateQueries({ queryKey: QUERY_KEYS.profileFields(workspaceId) });
    },
  });
};
