import { useQuery } from '@tanstack/react-query';
import { api } from '@/lib/api';
import { instanceManager } from '@/lib/instances';

export interface CustomEmoji {
  id: string;
  name: string;
  url: string;
  created_by: string;
  created_at: string;
}

export const customEmojiKey = (workspaceId: string) => ['custom-emoji', workspaceId] as const;

export function useCustomEmoji(workspaceId: string | undefined, instanceUrl?: string) {
  return useQuery({
    queryKey: customEmojiKey(workspaceId ?? ''),
    enabled: Boolean(workspaceId),
    // A workspace's emoji set changes a few times a year, and every message
    // rendered wants to resolve against it.
    staleTime: 5 * 60 * 1000,
    queryFn: async () => {
      const client = instanceUrl ? instanceManager.get(instanceUrl).api : api;
      const res = await client.get<{ data: CustomEmoji[] }>(`/workspaces/${workspaceId}/emojis`);
      return res.data;
    },
  });
}
