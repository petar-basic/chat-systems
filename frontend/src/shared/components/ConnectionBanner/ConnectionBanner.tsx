import { useInstanceStore } from '@/stores/instances';
import { useWsStatusStore } from '@/stores/wsStatus';
import { instanceManager } from '@/lib/instances';
import { ConnectionLabels } from '@/shared/constants';

interface Props {
  instanceUrl?: string;
}

export function ConnectionBanner({ instanceUrl }: Props) {
  const activeUrl = useInstanceStore((s) => s.activeInstanceUrl);
  const statuses = useWsStatusStore((s) => s.statuses);

  const url = instanceUrl ?? activeUrl ?? '';
  if (!url) return null;
  const status = statuses[instanceManager.normalize(url)] ?? statuses[url];

  if (!status || status === 'connected') return null;
  const offline = status === 'disconnected';

  return (
    <div
      role="status"
      data-qa="connection-banner"
      className={`px-4 py-1.5 text-xs flex items-center justify-center gap-2 shrink-0 border-b ${
        offline
          ? 'bg-red-500/10 text-red-300 border-red-500/20'
          : 'bg-amber-500/10 text-amber-300 border-amber-500/20'
      }`}
    >
      <span className={`w-1.5 h-1.5 rounded-full ${offline ? 'bg-red-400' : 'bg-amber-400'} animate-pulse`} />
      {offline ? ConnectionLabels.Offline : ConnectionLabels.Connecting}
    </div>
  );
}
