import { useMutation, useQuery } from '@tanstack/react-query';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';
import { logger } from '@/lib/logger';

export const useActiveHuddles = (workspaceId?: string | null, instanceUrl?: string) =>
  useQuery({
    queryKey: QUERY_KEYS.workspaceActiveHuddles(workspaceId ?? ''),
    queryFn: async () => {
      if (!workspaceId) throw new Error('No workspace selected');
      const res = await getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/workspaces/{ws_id}/active-huddles', { params: { path: { ws_id: workspaceId } } }),
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
      getApiForInstance(instanceUrl).typed((c) =>
        c.POST('/workspaces/{ws_id}/huddles', { params: { path: { ws_id: workspaceId } }, body }),
      ),
    onError: (err) => logger.error('useStartHuddle', 'mutationFn', err),
  });

export const useInviteToHuddle = (workspaceId: string, huddleId: string, instanceUrl?: string) =>
  useMutation({
    mutationFn: async (userIds: string[]) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.POST('/workspaces/{ws_id}/huddles/{huddle_id}/invite', {
          params: { path: { ws_id: workspaceId, huddle_id: huddleId } },
          body: { user_ids: userIds },
        }),
      ),
    onError: (err) => logger.error('useInviteToHuddle', 'mutationFn', err),
  });
