import type { InfiniteData, QueryClient } from '@tanstack/react-query';
import type { Message } from '@/stores/workspace';
import { QUERY_KEYS } from '@/shared/constants';
import { newestFirst, patchMessageById, upsertMessage } from './messageCache';
import { patchReply, placeReply } from './threadCache';

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
  if (message.thread_parent_id) patchReply(queryClient, message.id, updater);
}

export const shownInChannel = (message: Pick<Message, 'thread_parent_id' | 'metadata'>) =>
  !message.thread_parent_id || !!message.metadata?.also_in_channel;

export function placeOwnMessage(queryClient: QueryClient, message: Message) {
  placeReply(queryClient, message);
  if (!shownInChannel(message)) return;
  queryClient.setQueryData<ChannelMessages>(QUERY_KEYS.messages(message.channel_id), (cache) =>
    upsertMessage(cache, message, 'lastPage', newestFirst),
  );
}
