import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';

export type UserGroup = components['schemas']['UserGroupSummary'];

export const userGroupsKey = (workspaceId: string) => ['user-groups', workspaceId] as const;

export function useUserGroups(workspaceId: string | undefined, instanceUrl?: string) {
  return useQuery({
    queryKey: userGroupsKey(workspaceId ?? ''),
    enabled: Boolean(workspaceId),
    staleTime: 5 * 60 * 1000,
    queryFn: async () => {
      if (!workspaceId) throw new Error('No workspace selected');
      const res = await getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/workspaces/{ws_id}/groups', { params: { path: { ws_id: workspaceId } } }),
      );
      return res.data;
    },
  });
}

export function useCreateGroup(workspaceId: string, instanceUrl?: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: components['schemas']['CreateGroupRequest']) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.POST('/workspaces/{ws_id}/groups', { params: { path: { ws_id: workspaceId } }, body }),
      ),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: userGroupsKey(workspaceId) }),
  });
}

export function useDeleteGroup(workspaceId: string, instanceUrl?: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (groupId: string) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.DELETE('/workspaces/{ws_id}/groups/{group_id}', {
          params: { path: { ws_id: workspaceId, group_id: groupId } },
        }),
      ),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: userGroupsKey(workspaceId) }),
  });
}

export function useSetGroupMember(workspaceId: string, instanceUrl?: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ groupId, userId, member }: { groupId: string; userId: string; member: boolean }) =>
      member
        ? getApiForInstance(instanceUrl).typed((c) =>
            c.POST('/workspaces/{ws_id}/groups/{group_id}/members', {
              params: { path: { ws_id: workspaceId, group_id: groupId } },
              body: { user_id: userId },
            }),
          )
        : getApiForInstance(instanceUrl).typed((c) =>
            c.DELETE('/workspaces/{ws_id}/groups/{group_id}/members/{user_id}', {
              params: { path: { ws_id: workspaceId, group_id: groupId, user_id: userId } },
            }),
          ),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: userGroupsKey(workspaceId) }),
  });
}
