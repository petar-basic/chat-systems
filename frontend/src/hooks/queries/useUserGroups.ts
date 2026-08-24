import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api } from '@/lib/api';
import { instanceManager } from '@/lib/instances';

export interface UserGroup {
  id: string;
  handle: string;
  name: string;
  description: string | null;
  member_count: number;
  is_member: boolean;
}

export const userGroupsKey = (workspaceId: string) => ['user-groups', workspaceId] as const;

function clientFor(instanceUrl?: string) {
  return instanceUrl ? instanceManager.get(instanceUrl).api : api;
}

export function useUserGroups(workspaceId: string | undefined, instanceUrl?: string) {
  return useQuery({
    queryKey: userGroupsKey(workspaceId ?? ''),
    enabled: Boolean(workspaceId),
    staleTime: 5 * 60 * 1000,
    queryFn: async () => {
      const res = await clientFor(instanceUrl).get<{ data: UserGroup[] }>(
        `/workspaces/${workspaceId}/groups`,
      );
      return res.data;
    },
  });
}

export function useCreateGroup(workspaceId: string, instanceUrl?: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: { handle: string; name?: string; description?: string }) =>
      clientFor(instanceUrl).post<UserGroup>(`/workspaces/${workspaceId}/groups`, body),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: userGroupsKey(workspaceId) }),
  });
}

export function useDeleteGroup(workspaceId: string, instanceUrl?: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (groupId: string) =>
      clientFor(instanceUrl).delete(`/workspaces/${workspaceId}/groups/${groupId}`),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: userGroupsKey(workspaceId) }),
  });
}

export function useSetGroupMember(workspaceId: string, instanceUrl?: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ groupId, userId, member }: { groupId: string; userId: string; member: boolean }) =>
      member
        ? clientFor(instanceUrl).post(`/workspaces/${workspaceId}/groups/${groupId}/members`, {
            user_id: userId,
          })
        : clientFor(instanceUrl).delete(
            `/workspaces/${workspaceId}/groups/${groupId}/members/${userId}`,
          ),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: userGroupsKey(workspaceId) }),
  });
}
