import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { useWorkspaceStore } from '@/stores/workspace';
import { instanceManager } from '@/lib/instances';
import { api } from '@/lib/api';
import type { Message } from '@/stores/workspace';
import { QUERY_KEYS } from '@/shared/constants';

interface ThreadMessagesResponse {
  data: Message[];
}

function useCurrentApi() {
  const instanceUrl = useWorkspaceStore((s) => s.currentWorkspace?.instanceUrl);
  return instanceUrl ? instanceManager.get(instanceUrl).api : api;
}

export const useThreadMessages = (parentMessageId: string) => {
  const apiClient = useCurrentApi();
  return useQuery({
    queryKey: QUERY_KEYS.thread(parentMessageId),
    queryFn: async () => {
      const response = await apiClient.get<ThreadMessagesResponse>(`/messages/${parentMessageId}/thread`);
      return response.data;
    },
    staleTime: 1000 * 60,
  });
};

export const useSendThreadReply = (parentMessageId: string, channelId: string) => {
  const queryClient = useQueryClient();
  const apiClient = useCurrentApi();

  return useMutation({
    mutationFn: async (content: string) => {
      return apiClient.post<Message>(`/messages/${parentMessageId}/thread`, {
        content,
        channel_id: channelId,
      });
    },
    onSuccess: (newMessage) => {
      queryClient.setQueryData<Message[]>(['threads', parentMessageId], (old = []) => [...old, newMessage]);
      queryClient.invalidateQueries({ queryKey: QUERY_KEYS.messages(channelId) });
    },
  });
};
