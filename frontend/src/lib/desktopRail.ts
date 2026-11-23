import { useEffect } from 'react';
import { useQuery } from '@tanstack/react-query';
import { instanceManager } from '@/lib/instances';
import { desktopShell } from '@/lib/desktopShell';
import { instanceInfoQuery } from '@/lib/instanceInfo';
import { useInstanceStore } from '@/stores/instances';

export function useEmptyRailWhenSignedOut() {
  const origin = instanceManager.normalize(window.location.origin);
  const signedOut = useInstanceStore(
    (s) => s.hydrated && !s.instances.some((i) => instanceManager.normalize(i.url) === origin),
  );
  useEffect(() => {
    if (signedOut) desktopShell()?.reportWorkspaces([], null);
  }, [signedOut]);
}

export function useReportOtherServers() {
  const { data } = useQuery(instanceInfoQuery(window.location.origin));
  const allowed = data?.other_servers_allowed;
  useEffect(() => {
    if (allowed !== undefined) desktopShell()?.reportOtherServers?.(allowed);
  }, [allowed]);
}
