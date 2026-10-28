import type { QueryClient } from '@tanstack/react-query';
import { QUERY_KEYS } from '@/shared/constants';
import { useWorkspaceStore, type Channel } from '@/stores/workspace';

export function applyChannelArchived(
  queryClient: QueryClient,
  channelId: string,
  workspaceId: string,
  isArchived: boolean,
) {
  queryClient.setQueryData<Channel>(QUERY_KEYS.channel(channelId), (channel) =>
    channel ? { ...channel, is_archived: isArchived } : channel,
  );
  void queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaceChannels(workspaceId) });
  void queryClient.invalidateQueries({ queryKey: QUERY_KEYS.channelsBrowse(workspaceId) });
  const { currentChannel } = useWorkspaceStore.getState();
  if (currentChannel?.id === channelId && currentChannel.is_archived !== isArchived) {
    useWorkspaceStore.setState({ currentChannel: { ...currentChannel, is_archived: isArchived } });
  }
}

export function useChannelArchived(channelId: string | null | undefined): boolean {
  return useWorkspaceStore(
    (s) => !!channelId && s.currentChannel?.id === channelId && !!s.currentChannel.is_archived,
  );
}
