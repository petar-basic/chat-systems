import { Phone } from 'lucide-react';
import { useWorkspaceStore } from '@/stores/workspace';
import { useHuddleStore } from '@/stores/huddle';
import { useUserCache } from '@/stores/users';
import { displayNameOf } from '@/lib/userHelpers';

interface Props {
  channelId: string;
  huddleId: string;
  initiatorId: string;
}

export function HuddleSystemMessage({ channelId, huddleId, initiatorId }: Props) {
  const { getUser } = useUserCache();
  const name = displayNameOf(getUser(initiatorId)?.display_name);
  const channelHuddle = useWorkspaceStore((s) => s.activeHuddleChannels.get(channelId));
  const currentWorkspace = useWorkspaceStore((s) => s.currentWorkspace);
  const currentUserId = useWorkspaceStore((s) => s.currentUserId);
  const active = useHuddleStore((s) => s.active);

  const stillActive = channelHuddle?.huddleId === huddleId;
  const inThisHuddle = active?.scope.kind === 'channel' && active.scope.channelId === channelId;
  const canJoin = stillActive && !inThisHuddle && !active && !!currentWorkspace && !!currentUserId;

  const join = () => {
    if (!canJoin) return;
    useHuddleStore.getState().setActive({
      huddleId,
      workspaceId: currentWorkspace.id,
      instanceUrl: currentWorkspace.instanceUrl,
      selfUserId: currentUserId,
      scope: { kind: 'channel', channelId },
    });
  };

  return (
    <div className="flex items-center gap-2 py-1.5 px-2 text-sm text-slate-400">
      <Phone className="w-3.5 h-3.5 text-purple-400 shrink-0" />
      <span>
        <span className="text-slate-200 font-medium">{name}</span> started a huddle
      </span>
      {canJoin && (
        <button
          onClick={join}
          className="ml-1 px-2 py-0.5 rounded-md bg-green-600/20 text-green-300 hover:bg-green-600/30 text-xs font-medium transition"
        >
          Join
        </button>
      )}
    </div>
  );
}
