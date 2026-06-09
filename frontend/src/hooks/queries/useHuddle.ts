import { useMutation, useQuery } from '@tanstack/react-query';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';
import { logger } from '@/lib/logger';

export interface ActiveHuddle {
  huddle_id: string;
  channel_id: string;
  initiator_id: string;
}

export const useActiveHuddles = (workspaceId?: string | null, instanceUrl?: string) =>
  useQuery({
    queryKey: QUERY_KEYS.workspaceActiveHuddles(workspaceId ?? ''),
    queryFn: async () => {
      const res = await getApiForInstance(instanceUrl).get<{ data: ActiveHuddle[] }>(
        `/workspaces/${workspaceId}/active-huddles`,
      );
      return res.data;
    },
    enabled: !!workspaceId && !!instanceUrl,
    staleTime: Infinity,
    refetchOnWindowFocus: false,
  });

type StartHuddleBody = { channel_id: string } | { dm_partner_id: string };

export const useStartHuddle = (workspaceId: string, instanceUrl?: string) =>
  useMutation({
    mutationFn: async (body: StartHuddleBody) =>
      getApiForInstance(instanceUrl).post<{ huddle_id: string }>(`/workspaces/${workspaceId}/huddles`, body),
    onError: (err) => logger.error('useStartHuddle', 'mutationFn', err),
  });

export const useInviteToHuddle = (workspaceId: string, huddleId: string, instanceUrl?: string) =>
  useMutation({
    mutationFn: async (userIds: string[]) =>
      getApiForInstance(instanceUrl).post(`/workspaces/${workspaceId}/huddles/${huddleId}/invite`, {
        user_ids: userIds,
      }),
    onError: (err) => logger.error('useInviteToHuddle', 'mutationFn', err),
  });
