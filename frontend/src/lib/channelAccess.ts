import type { QueryClient } from '@tanstack/react-query';
import { QUERY_KEYS } from '@/shared/constants';
import { useWorkspaceStore, type Channel } from '@/stores/workspace';

export function forgetChannel(queryClient: QueryClient, channelId: string, workspaceId: string | null) {
  queryClient.removeQueries({ queryKey: QUERY_KEYS.messages(channelId) });
  queryClient.removeQueries({ queryKey: QUERY_KEYS.channelMembers(channelId) });
  if (workspaceId) {
    queryClient.setQueryData<Channel[]>(QUERY_KEYS.workspaceChannels(workspaceId), (channels) =>
      channels?.filter((ch) => ch.id !== channelId),
    );
    void queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaceChannels(workspaceId) });
  }
  if (useWorkspaceStore.getState().currentChannel?.id === channelId) {
    useWorkspaceStore.setState({ currentChannel: null });
  }
}

export interface PrivacyChange {
  channel_id: string;
  workspace_id: string;
  private: boolean;
}

export function applyChannelPrivacy(queryClient: QueryClient, change: PrivacyChange) {
  void queryClient.invalidateQueries({ queryKey: QUERY_KEYS.channelsBrowse(change.workspace_id) });
  const mine = queryClient
    .getQueryData<Channel[]>(QUERY_KEYS.workspaceChannels(change.workspace_id))
    ?.some((ch) => ch.id === change.channel_id);
  if (change.private && !mine) {
    queryClient.removeQueries({ queryKey: QUERY_KEYS.channel(change.channel_id) });
    forgetChannel(queryClient, change.channel_id, change.workspace_id);
  }
}
