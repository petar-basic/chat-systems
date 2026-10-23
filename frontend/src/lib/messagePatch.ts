import type { InfiniteData, QueryClient } from '@tanstack/react-query';
import type { Message } from '@/stores/workspace';
import { QUERY_KEYS } from '@/shared/constants';
import { newestFirst, patchMessageById, upsertMessage } from './messageCache';

type ChannelMessages = InfiniteData<{ data: Message[] }>;
type Located = Pick<Message, 'id' | 'channel_id' | 'thread_parent_id'>;

export function patchMessageEverywhere(
  queryClient: QueryClient,
  message: Located,
  updater: (message: Message) => Message,
) {
  queryClient.setQueryData<ChannelMessages>(QUERY_KEYS.messages(message.channel_id), (cache) =>
    patchMessageById(cache, message.id, updater),
  );
  if (message.thread_parent_id) {
    queryClient.setQueryData<Message[]>(QUERY_KEYS.thread(message.thread_parent_id), (thread) =>
      thread?.map((m) => (m.id === message.id ? updater(m) : m)),
    );
  }
}

export function placeOwnMessage(queryClient: QueryClient, message: Message) {
  if (message.thread_parent_id) {
    queryClient.setQueryData<Message[]>(QUERY_KEYS.thread(message.thread_parent_id), (thread) =>
      thread && !thread.some((m) => m.id === message.id) ? [...thread, message] : thread,
    );
    return;
  }
  queryClient.setQueryData<ChannelMessages>(QUERY_KEYS.messages(message.channel_id), (cache) =>
    upsertMessage(cache, message, 'lastPage', newestFirst),
  );
}
