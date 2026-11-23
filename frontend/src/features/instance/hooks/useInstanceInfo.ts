import { useQuery } from '@tanstack/react-query';
import { instanceInfoQuery } from '@/lib/instanceInfo';

export { instanceInfoQuery };

export function useInstanceInfo(instanceUrl: string | undefined) {
  return useQuery(instanceInfoQuery(instanceUrl));
}

export function useOtherServersAllowed() {
  const { data } = useInstanceInfo(window.location.origin);
  return data?.other_servers_allowed === true;
}
