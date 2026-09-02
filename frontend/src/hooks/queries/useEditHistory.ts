import { useQuery } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';

export type MessageEdit = components['schemas']['MessageEdit'];

export const useEditHistory = (messageId: string | null, instanceUrl?: string) => {
  return useQuery({
    queryKey: QUERY_KEYS.editHistory(messageId ?? ''),
    queryFn: async () => {
      if (!messageId) throw new Error('No message ID');
      const res = await getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/messages/{msg_id}/history', { params: { path: { msg_id: messageId } } }),
      );
      return res.data;
    },
    enabled: !!messageId,
    staleTime: 1000 * 30,
  });
};
