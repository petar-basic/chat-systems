import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';
import { applyChannelArchived } from '@/lib/channelArchive';

export type WorkspaceAnalytics = components['schemas']['WorkspaceAnalytics'];
export type DailyActivity = components['schemas']['DailyActivity'];
export type MemberActivity = components['schemas']['MemberActivity'];
export type ChannelActivity = components['schemas']['ChannelActivity'];
export type QuietChannel = components['schemas']['QuietChannel'];
export type AnalyticsWindow = 7 | 30 | 90;
export type QuietThreshold = 30 | 60 | 90 | 180;

export const ANALYTICS_WINDOWS: AnalyticsWindow[] = [7, 30, 90];
export const QUIET_THRESHOLDS: QuietThreshold[] = [30, 60, 90, 180];

export const useWorkspaceAnalytics = (
  workspaceId: string,
  days: AnalyticsWindow,
  guests: boolean,
  instanceUrl?: string,
) =>
  useQuery({
    queryKey: QUERY_KEYS.analytics(workspaceId, days, guests),
    queryFn: () =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/workspaces/{ws_id}/analytics', {
          params: { path: { ws_id: workspaceId }, query: { days, guests } },
        }),
      ),
  });

export const useMemberAnalytics = (
  workspaceId: string,
  days: AnalyticsWindow,
  guests: boolean,
  instanceUrl?: string,
) =>
  useQuery({
    queryKey: QUERY_KEYS.analyticsMembers(workspaceId, days, guests),
    queryFn: async () => {
      const res = await getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/workspaces/{ws_id}/analytics/members', {
          params: { path: { ws_id: workspaceId }, query: { days, guests } },
        }),
      );
      return res.data;
    },
  });

export const useChannelActivity = (
  workspaceId: string,
  days: AnalyticsWindow,
  guests: boolean,
  instanceUrl?: string,
) =>
  useQuery({
    queryKey: QUERY_KEYS.analyticsChannels(workspaceId, days, guests),
    queryFn: async () => {
      const res = await getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/workspaces/{ws_id}/analytics/channels', {
          params: { path: { ws_id: workspaceId }, query: { days, guests } },
        }),
      );
      return res.data;
    },
  });

export const useQuietChannels = (workspaceId: string, days: QuietThreshold, instanceUrl?: string) =>
  useQuery({
    queryKey: QUERY_KEYS.quietChannels(workspaceId, days),
    queryFn: async () => {
      const res = await getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/workspaces/{ws_id}/analytics/quiet-channels', {
          params: { path: { ws_id: workspaceId }, query: { days } },
        }),
      );
      return res.data;
    },
  });

export const useArchiveQuietChannels = (workspaceId: string, instanceUrl?: string) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (channelIds: string[]) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.POST('/workspaces/{ws_id}/analytics/quiet-channels/archive', {
          params: { path: { ws_id: workspaceId } },
          body: { channel_ids: channelIds },
        }),
      ),
    onSuccess: ({ archived }) => {
      for (const id of archived) applyChannelArchived(queryClient, id, workspaceId, true);
      void queryClient.invalidateQueries({ queryKey: ['analytics', workspaceId] });
    },
  });
};
