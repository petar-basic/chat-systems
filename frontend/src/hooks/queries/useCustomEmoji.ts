import { useQuery } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';

export type CustomEmoji = components['schemas']['EmojiView'];

export const customEmojiKey = (workspaceId: string) => ['custom-emoji', workspaceId] as const;

export function useCustomEmoji(workspaceId: string | undefined, instanceUrl?: string) {
  return useQuery({
    queryKey: customEmojiKey(workspaceId ?? ''),
    enabled: Boolean(workspaceId),
    // A workspace's emoji set changes a few times a year, and every message
    // rendered wants to resolve against it.
    staleTime: 5 * 60 * 1000,
    queryFn: async () => {
      if (!workspaceId) throw new Error('No workspace selected');
      const res = await getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/workspaces/{ws_id}/emojis', { params: { path: { ws_id: workspaceId } } }),
      );
      return res.data;
    },
  });
}
