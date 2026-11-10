import { queryOptions, useQuery } from '@tanstack/react-query';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';

export const instanceInfoQuery = (instanceUrl: string | undefined) =>
  queryOptions({
    queryKey: QUERY_KEYS.instanceInfo(instanceUrl ?? ''),
    queryFn: () => getApiForInstance(instanceUrl).typed((c) => c.GET('/instance/info')),
    staleTime: 60 * 60 * 1000,
  });

export function useInstanceInfo(instanceUrl: string | undefined) {
  return useQuery(instanceInfoQuery(instanceUrl));
}
