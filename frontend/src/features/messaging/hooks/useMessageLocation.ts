import { useCallback } from 'react';
import { useNavigate } from 'react-router';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import type { ApiClient } from '@/lib/api';
import { useCurrentApi } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS, ROUTES } from '@/shared/constants';
import { toast } from '@/shared/components/Toast';

export type MessageLocation = components['schemas']['MessageLocation'];

const locate = (apiClient: ApiClient, messageId: string) => ({
  queryKey: QUERY_KEYS.messageLocation(messageId),
  queryFn: () =>
    apiClient.typed((c) => c.GET('/messages/{msg_id}/location', { params: { path: { msg_id: messageId } } })),
  staleTime: 1000 * 60 * 5,
  retry: false,
});

export function messagePath(location: MessageLocation, messageId: string, thread = false) {
  const path =
    location.kind === 'channel'
      ? ROUTES.message(location.workspace_id, location.channel_id, messageId)
      : ROUTES.conversationMessage(location.workspace_id, location.channel_id, messageId);
  return thread ? `${path}?thread=1` : path;
}

export const useMessageLocation = (messageId: string | undefined) => {
  const apiClient = useCurrentApi();
  return useQuery({ ...locate(apiClient, messageId ?? ''), enabled: !!messageId });
};

export function useOpenMessage() {
  const apiClient = useCurrentApi();
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  return useCallback(
    async (messageId: string, thread = false) => {
      try {
        const location = await queryClient.fetchQuery(locate(apiClient, messageId));
        navigate(messagePath(location, messageId, thread));
      } catch {
        toast.error('That message was deleted or you can no longer see it');
      }
    },
    [apiClient, queryClient, navigate],
  );
}
