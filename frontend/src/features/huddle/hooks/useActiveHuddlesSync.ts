import { useEffect } from 'react';
import { useActiveHuddles } from '@/hooks/queries/useHuddle';
import { useWorkspaceStore } from '@/stores/workspace';

export function useActiveHuddlesSync(workspaceId?: string | null, instanceUrl?: string) {
  const { data } = useActiveHuddles(workspaceId, instanceUrl);

  useEffect(() => {
    if (!data) return;
    useWorkspaceStore.getState().replaceActiveHuddleChannels(
      data.map((h) => ({
        channelId: h.channel_id,
        huddleId: h.huddle_id,
        initiatorId: h.initiator_id,
      })),
    );
  }, [data]);
}
