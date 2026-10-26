import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';

export type ScimToken = components['schemas']['ScimToken'];
export type MintedScimToken = components['schemas']['MintedScimToken'];

export const useScimTokens = (instanceUrl?: string) =>
  useQuery({
    queryKey: QUERY_KEYS.scimTokens(instanceUrl ?? ''),
    queryFn: async () => {
      const res = await getApiForInstance(instanceUrl).typed((c) => c.GET('/admin/scim/tokens'));
      return res.data;
    },
  });

function useTokenMutation<T>(instanceUrl: string | undefined, run: (value: T) => Promise<MintedScimToken>) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: run,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: QUERY_KEYS.scimTokens(instanceUrl ?? '') }),
  });
}

export const useMintScimToken = (instanceUrl?: string) =>
  useTokenMutation(instanceUrl, (description: string) =>
    getApiForInstance(instanceUrl).typed((c) =>
      c.POST('/admin/scim/tokens', { body: { description: description || null } }),
    ),
  );

export const useRotateScimToken = (instanceUrl?: string) =>
  useTokenMutation(instanceUrl, (id: string) =>
    getApiForInstance(instanceUrl).typed((c) =>
      c.POST('/admin/scim/tokens/{id}/rotate', { params: { path: { id } } }),
    ),
  );

export const useRevokeScimToken = (instanceUrl?: string) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.DELETE('/admin/scim/tokens/{id}', { params: { path: { id } } }),
      ),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: QUERY_KEYS.scimTokens(instanceUrl ?? '') }),
  });
};
