import { Phone, Users } from 'lucide-react';
import { useWorkspaceStore } from '@/stores/workspace';
import { useHuddleStore } from '@/stores/huddle';
import { useStartHuddle } from '@/hooks/queries/useHuddle';

export function HuddleBar({ channelId }: { channelId: string }) {
  const currentWorkspace = useWorkspaceStore((s) => s.currentWorkspace);
  const currentUserId = useWorkspaceStore((s) => s.currentUserId);
  const channelHuddle = useWorkspaceStore((s) => s.activeHuddleChannels.get(channelId));
  const active = useHuddleStore((s) => s.active);

  const start = useStartHuddle(currentWorkspace?.id ?? '', currentWorkspace?.instanceUrl);

  if (!currentWorkspace || !currentUserId) return null;

  const inThisHuddle = active?.scope.kind === 'channel' && active.scope.channelId === channelId;
  if (inThisHuddle) return null;

  const busy = !!active || start.isPending;

  const enter = (huddleId: string) => {
    useHuddleStore.getState().setActive({
      huddleId,
      workspaceId: currentWorkspace.id,
      instanceUrl: currentWorkspace.instanceUrl,
      selfUserId: currentUserId,
      scope: { kind: 'channel', channelId },
    });
  };

  const onStart = async () => {
    if (busy) return;
    const res = await start.mutateAsync({ channel_id: channelId }).catch(() => null);
    if (res) enter(res.huddle_id);
  };

  if (channelHuddle) {
    return (
      <button
        onClick={() => !busy && enter(channelHuddle.huddleId)}
        disabled={busy}
        aria-label="Join huddle"
        className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg bg-green-600/20 text-success hover:bg-green-600/30 transition cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed"
      >
        <Users className="w-4 h-4" />
        <span className="text-xs font-medium">Join huddle</span>
      </button>
    );
  }

  return (
    <button
      onClick={onStart}
      disabled={busy}
      aria-label="Start huddle"
      title="Start huddle"
      className="p-1.5 rounded-lg text-muted hover:text-fg hover:bg-raised/50 transition cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed"
    >
      <Phone className="w-4 h-4" />
    </button>
  );
}
