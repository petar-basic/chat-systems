import type { InfiniteData, QueryClient } from '@tanstack/react-query';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { useWorkspaceStore, type Message } from '@/stores/workspace';
import { QUERY_KEYS } from '@/shared/constants';
import { logger } from './logger';

const READ_DELAY_MS = 250;
const pending = new Map<string, ReturnType<typeof setTimeout>>();

export function newestSentIn(queryClient: QueryClient, channelId: string): string | null {
  const cache = queryClient.getQueryData<InfiniteData<{ data: Message[] }>>(QUERY_KEYS.messages(channelId));
  let newest: Message | null = null;
  for (const page of cache?.pages ?? []) {
    for (const message of page.data) {
      if (message.pending || message.failed) continue;
      if (!newest || message.created_at > newest.created_at) newest = message;
    }
  }
  return newest?.id ?? null;
}

export function onScreen(channelId: string): 'channel' | 'conversation' | null {
  if (document.visibilityState !== 'visible' || !document.hasFocus()) return null;
  const { currentChannel, currentConversationId } = useWorkspaceStore.getState();
  if (currentChannel?.id === channelId) return 'channel';
  if (currentConversationId === channelId) return 'conversation';
  return null;
}

export function readOnScreen(queryClient: QueryClient, channelId: string) {
  const where = onScreen(channelId);
  if (!where) return;
  const instanceUrl = useWorkspaceStore.getState().currentWorkspace?.instanceUrl;
  clearTimeout(pending.get(channelId));
  pending.set(
    channelId,
    setTimeout(() => {
      pending.delete(channelId);
      const api = getApiForInstance(instanceUrl);
      if (where === 'conversation') {
        api
          .typed((c) => c.POST('/conversations/{conv_id}/read', { params: { path: { conv_id: channelId } } }))
          .catch((err) => logger.error('readOnScreen', where, err));
        return;
      }
      const newest = newestSentIn(queryClient, channelId);
      if (!newest) return;
      api
        .typed((c) =>
          c.POST('/channels/{ch_id}/read', {
            params: { path: { ch_id: channelId } },
            body: { message_id: newest },
          }),
        )
        .catch((err) => logger.error('readOnScreen', where, err));
    }, READ_DELAY_MS),
  );
}
