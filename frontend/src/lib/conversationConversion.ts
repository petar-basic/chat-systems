import type { QueryClient } from '@tanstack/react-query';
import { QUERY_KEYS } from '@/shared/constants';
import type { Channel } from '@/stores/workspace';
import type { Conversation } from '@/features/messaging/hooks/useConversations';

export function applyConversion(queryClient: QueryClient, channel: Channel) {
  const workspaceId = channel.workspace_id;
  queryClient.setQueryData<Conversation[]>(QUERY_KEYS.conversations(workspaceId), (conversations) =>
    conversations?.filter((c) => c.id !== channel.id),
  );
  queryClient.setQueryData<Channel[]>(QUERY_KEYS.workspaceChannels(workspaceId), (channels) =>
    channels && !channels.some((c) => c.id === channel.id) ? [...channels, channel] : channels,
  );
  void queryClient.invalidateQueries({ queryKey: QUERY_KEYS.conversations(workspaceId) });
  void queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaceChannels(workspaceId) });
}
