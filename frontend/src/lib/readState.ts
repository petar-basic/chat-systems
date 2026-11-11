import type { QueryClient } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import type { Conversation } from '@/features/messaging/hooks/useConversations';
import { useWorkspaceStore } from '@/stores/workspace';
import { QUERY_KEYS } from '@/shared/constants';

export interface ReadStateUpdate {
  channel_id: string;
  unread_count: number;
  mention_count: number;
}

export function applyReadState(queryClient: QueryClient, update: ReadStateUpdate) {
  const store = useWorkspaceStore.getState();
  const conversationsKey = QUERY_KEYS.conversations(store.currentWorkspace?.id ?? '');
  const conversations = queryClient.getQueryData<Conversation[]>(conversationsKey);
  if (conversations?.some((c) => c.id === update.channel_id)) {
    queryClient.setQueryData<Conversation[]>(conversationsKey, (old) =>
      old?.map((c) => (c.id === update.channel_id ? { ...c, unread_count: update.unread_count } : c)),
    );
    if (update.unread_count > 0) store.markConversationUnread(update.channel_id);
    else store.markConversationRead(update.channel_id);
    return;
  }
  store.setChannelReadState(update.channel_id, update.unread_count, update.mention_count);
}

type UnreadChannels = components['schemas']['UnreadChannels'];

export async function recountAfterReplay(queryClient: QueryClient, workspaceId: string) {
  const here = () => useWorkspaceStore.getState().currentWorkspace?.id === workspaceId;
  if (!here()) return;
  await Promise.all([
    queryClient.refetchQueries({ queryKey: QUERY_KEYS.channelsUnread(workspaceId) }),
    queryClient.refetchQueries({ queryKey: QUERY_KEYS.conversations(workspaceId) }),
  ]);
  const unread = queryClient.getQueryData<UnreadChannels>(QUERY_KEYS.channelsUnread(workspaceId));
  if (!unread || !here()) return;
  const store = useWorkspaceStore.getState();
  store.hydrateUnreadChannels(unread.channel_ids);
  store.hydrateUnreadCounts(unread.counts);
}
