import { useSyncExternalStore } from 'react';
import type { InfiniteData, QueryClient } from '@tanstack/react-query';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { useWorkspaceStore, type Message } from '@/stores/workspace';
import { QUERY_KEYS } from '@/shared/constants';
import { isKeptUnread } from '@/stores/keptUnread';
import { logger } from './logger';

const READ_DELAY_MS = 250;
const pending = new Map<string, ReturnType<typeof setTimeout>>();
const sent = new Map<string, string>();

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

export function windowInUse(): boolean {
  return document.visibilityState === 'visible' && document.hasFocus();
}

function onUseChange(listener: () => void): () => void {
  window.addEventListener('focus', listener);
  window.addEventListener('blur', listener);
  document.addEventListener('visibilitychange', listener);
  return () => {
    window.removeEventListener('focus', listener);
    window.removeEventListener('blur', listener);
    document.removeEventListener('visibilitychange', listener);
  };
}

export function useWindowInUse(): boolean {
  return useSyncExternalStore(onUseChange, windowInUse);
}

export function noteRead(channelId: string, messageId: string) {
  sent.set(channelId, messageId);
}

export function onScreen(channelId: string): 'channel' | 'conversation' | null {
  if (!windowInUse()) return null;
  const { currentChannel, currentConversationId } = useWorkspaceStore.getState();
  if (currentChannel?.id === channelId) return 'channel';
  if (currentConversationId === channelId) return 'conversation';
  return null;
}

export function readOnScreen(queryClient: QueryClient, channelId: string) {
  const where = onScreen(channelId);
  if (!where || isKeptUnread(channelId)) return;
  const instanceUrl = useWorkspaceStore.getState().currentWorkspace?.instanceUrl;
  clearTimeout(pending.get(channelId));
  pending.set(
    channelId,
    setTimeout(() => {
      pending.delete(channelId);
      const newest = newestSentIn(queryClient, channelId);
      if (!newest) return;
      sent.set(channelId, newest);
      getApiForInstance(instanceUrl)
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

export function readOnReturn(queryClient: QueryClient): () => void {
  const onReturn = () => {
    const { currentChannel, currentConversationId } = useWorkspaceStore.getState();
    for (const channelId of [currentChannel?.id, currentConversationId]) {
      if (!channelId) continue;
      const newest = newestSentIn(queryClient, channelId);
      if (newest && sent.get(channelId) !== newest) readOnScreen(queryClient, channelId);
    }
  };
  window.addEventListener('focus', onReturn);
  document.addEventListener('visibilitychange', onReturn);
  return () => {
    window.removeEventListener('focus', onReturn);
    document.removeEventListener('visibilitychange', onReturn);
  };
}
