import type { InfiniteData, QueryClient } from '@tanstack/react-query';
import type { Message } from '@/stores/workspace';
import { unsentScope, useUnsentStore } from '@/stores/unsent';
import { restoreDraft } from '@/stores/drafts';
import { restoreAttachments } from '@/features/messaging/useComposerAttachments';
import { QUERY_KEYS } from '@/shared/constants';
import { removeMessageById } from './messageCache';
import { dropReply } from './threadCache';

export function discardUnsent(queryClient: QueryClient, message: Message, draftKey: string) {
  useUnsentStore.getState().drop(message);
  if (message.thread_parent_id) dropReply(queryClient, message.id);
  else
    queryClient.setQueryData<InfiniteData<{ data: Message[] }>>(
      QUERY_KEYS.messages(message.channel_id),
      (cache) => removeMessageById(cache, message.id),
    );
  restoreAttachments(unsentScope(message), message.metadata?.attachments ?? []);
  if (message.content.trim()) restoreDraft(draftKey, message.content);
}
