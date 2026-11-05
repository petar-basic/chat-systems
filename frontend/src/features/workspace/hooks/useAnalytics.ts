import { useQuery } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';

export type WorkspaceAnalytics = components['schemas']['WorkspaceAnalytics'];
export type DailyActivity = components['schemas']['DailyActivity'];
export type MemberActivity = components['schemas']['MemberActivity'];
export type AnalyticsWindow = 7 | 30 | 90;

export const ANALYTICS_WINDOWS: AnalyticsWindow[] = [7, 30, 90];

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
