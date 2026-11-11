import { useEffect } from 'react';
import { instanceManager } from '@/lib/instances';
import { desktopShell } from '@/lib/desktopShell';
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
