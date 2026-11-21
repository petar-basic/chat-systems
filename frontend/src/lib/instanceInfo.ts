import { queryOptions } from '@tanstack/react-query';
import { QUERY_KEYS } from '@/shared/constants';
import { api } from './api';
import { instanceManager } from './instances';
import { queryClient } from './queryClient';

export const instanceInfoQuery = (instanceUrl: string | undefined) =>
  queryOptions({
    queryKey: QUERY_KEYS.instanceInfo(instanceUrl ?? ''),
    queryFn: () =>
      (instanceUrl ? instanceManager.get(instanceUrl).api : api).typed((c) => c.GET('/instance/info')),
    staleTime: 60 * 60 * 1000,
  });

export async function otherServersAllowed(): Promise<boolean> {
  try {
    const info = await queryClient.fetchQuery(instanceInfoQuery(window.location.origin));
    return info.other_servers_allowed;
  } catch {
    return true;
  }
}
