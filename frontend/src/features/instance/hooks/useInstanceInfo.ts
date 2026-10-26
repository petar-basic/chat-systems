import { useQuery } from '@tanstack/react-query';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';

export function useInstanceInfo(instanceUrl: string | undefined) {
  return useQuery({
    queryKey: QUERY_KEYS.instanceInfo(instanceUrl ?? ''),
    queryFn: () => getApiForInstance(instanceUrl).typed((c) => c.GET('/instance/info')),
    staleTime: 60 * 60 * 1000,
  });
}
