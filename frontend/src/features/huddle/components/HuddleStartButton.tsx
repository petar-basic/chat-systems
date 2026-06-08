import { Phone } from 'lucide-react';
import { useStartHuddle } from '@/hooks/queries/useHuddle';
import { useHuddleStore } from '@/stores/huddle';

interface Props {
  workspaceId: string;
  instanceUrl: string;
  partnerId: string;
  currentUserId: string;
}

export function HuddleStartButton({ workspaceId, instanceUrl, partnerId, currentUserId }: Props) {
  const active = useHuddleStore((s) => s.active);
  const start = useStartHuddle(workspaceId, instanceUrl);
  const inHuddle = !!active;

  const onClick = async () => {
    if (inHuddle || start.isPending) return;
    const res = await start.mutateAsync({ dm_partner_id: partnerId }).catch(() => null);
    if (!res) return;
    useHuddleStore.getState().setActive({
      huddleId: res.huddle_id,
      workspaceId,
      instanceUrl,
      selfUserId: currentUserId,
      scope: { kind: 'dm', partnerId },
    });
  };

  return (
    <button
      onClick={onClick}
      disabled={inHuddle || start.isPending}
      aria-label="Start huddle"
      title="Start huddle"
      className="p-1.5 rounded-lg text-slate-400 hover:text-white hover:bg-slate-700/50 transition cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed"
    >
      <Phone className="w-4 h-4" />
    </button>
  );
}
