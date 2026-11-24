import { useQuery } from '@tanstack/react-query';
import { instanceInfoQuery } from '@/lib/instanceInfo';
import { inPhoneApp } from '@/lib/phoneApp';

export { instanceInfoQuery };

export function useInstanceInfo(instanceUrl: string | undefined) {
  return useQuery(instanceInfoQuery(instanceUrl));
}

export function useOtherServersAllowed() {
  const { data, isError } = useInstanceInfo(window.location.origin);
  return inPhoneApp || isError || data?.other_servers_allowed === true;
}
