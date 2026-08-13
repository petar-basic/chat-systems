import { useQuery } from '@tanstack/react-query';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';

export interface MessageEdit {
  id: string;
  message_id: string;
  previous_content: string;
  edited_by: string;
  edited_at: string;
}

export const useEditHistory = (
  messageId: string | null,
  scope: 'channel' | 'conversation',
  instanceUrl?: string,
) => {
  const path =
    scope === 'channel' ? `/messages/${messageId}/history` : `/conversations/messages/${messageId}/history`;

  return useQuery({
    queryKey: QUERY_KEYS.editHistory(messageId ?? ''),
    queryFn: async () => {
      const res = await getApiForInstance(instanceUrl).get<{ data: MessageEdit[] }>(path);
      return res.data;
    },
    enabled: !!messageId,
    staleTime: 1000 * 30,
  });
};
