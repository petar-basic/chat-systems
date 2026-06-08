import { useMutation } from '@tanstack/react-query';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { logger } from '@/lib/logger';

type StartHuddleBody = { channel_id: string } | { dm_partner_id: string };

export const useStartHuddle = (workspaceId: string, instanceUrl?: string) =>
  useMutation({
    mutationFn: async (body: StartHuddleBody) =>
      getApiForInstance(instanceUrl).post<{ huddle_id: string }>(
        `/workspaces/${workspaceId}/huddles`,
        body,
      ),
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
