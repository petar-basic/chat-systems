import { useMutation } from '@tanstack/react-query';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { usePresenceStore } from '@/stores/presence';
import { useWorkspaceStore } from '@/stores/workspace';
import { toast } from '@/shared/components/Toast';
import { toUserMessage } from '@/lib/errors';
import { logger } from '@/lib/logger';

export const useSetAway = (userId: string) => {
  const instanceUrl = useWorkspaceStore((s) => s.currentWorkspace?.instanceUrl);
  const setStatus = usePresenceStore((s) => s.setStatus);

  return useMutation({
    mutationFn: async (away: boolean) =>
      getApiForInstance(instanceUrl).typed((c) => c.PUT('/users/me/presence', { body: { away } })),
    onMutate: (away) => {
      const previous = usePresenceStore.getState().getStatus(userId);
      setStatus(userId, away ? 'away' : 'online');
      return { previous };
    },
    onError: (err, _away, ctx) => {
      logger.error('useSetAway', 'mutationFn', err);
      if (ctx) setStatus(userId, ctx.previous);
      toast.error(toUserMessage(err, 'Could not change your status'));
    },
  });
};
