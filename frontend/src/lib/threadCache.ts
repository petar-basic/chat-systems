import type { InfiniteData, QueryClient } from '@tanstack/react-query';
import type { Message } from '@/stores/workspace';
import { QUERY_KEYS } from '@/shared/constants';
import { hasMessage, newestFirst, patchMessageById, removeMessageById, upsertMessage } from './messageCache';

export type ThreadPageOfReplies = { data: Message[]; next?: string | null };
export type ThreadReplies = InfiniteData<ThreadPageOfReplies>;

export const oldestFirst = (cache: ThreadReplies | undefined): Message[] =>
  cache ? cache.pages.flatMap((p) => p.data).reverse() : [];

export function placeReply(queryClient: QueryClient, reply: Message, replacing?: string) {
  if (!reply.thread_parent_id) return;
  queryClient.setQueryData<ThreadReplies>(QUERY_KEYS.thread(reply.thread_parent_id), (cache) =>
    upsertMessage(replacing ? removeMessageById(cache, replacing) : cache, reply, 'firstPage', newestFirst),
  );
}

export function patchReply(
  queryClient: QueryClient,
  messageId: string,
  updater: (message: Message) => Message,
) {
  for (const [key, cache] of queryClient.getQueriesData<ThreadReplies>({
    queryKey: QUERY_KEYS.threadsAll(),
  })) {
    if (hasMessage(cache, messageId))
      queryClient.setQueryData(key, patchMessageById(cache, messageId, updater));
  }
}

export function dropReply(queryClient: QueryClient, messageId: string) {
  for (const [key, cache] of queryClient.getQueriesData<ThreadReplies>({
    queryKey: QUERY_KEYS.threadsAll(),
  })) {
    if (hasMessage(cache, messageId)) queryClient.setQueryData(key, removeMessageById(cache, messageId));
  }
}
