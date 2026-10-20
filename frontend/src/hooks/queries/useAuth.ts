import { useMemo } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useInstanceStore, type InstanceUser, type InstanceConfig, toInstanceUser } from '@/stores/instances';
import { instanceManager } from '@/lib/instances';
import { useWsStatusStore } from '@/stores/wsStatus';

export type User = InstanceUser;

export const useCurrentUser = () => {
  const instances = useInstanceStore((s) => s.instances);
  const activeInstanceUrl = useInstanceStore((s) => s.activeInstanceUrl);
  const hydrated = useInstanceStore((s) => s.hydrated);

  const user = useMemo(() => {
    if (activeInstanceUrl) {
      const instance = instances.find((i) => i.url === activeInstanceUrl);
      if (instance) return instance.user;
    }
    return instances[0]?.user ?? null;
  }, [instances, activeInstanceUrl]);

  return { data: user, isLoading: !hydrated };
};

export const useAddInstance = () => {
  const { addInstance } = useInstanceStore();

  return useMutation({
    mutationFn: async ({
      url,
      email,
      password,
      wsUrl,
      totpCode,
    }: {
      url: string;
      email: string;
      password: string;
      wsUrl?: string;
      totpCode?: string;
    }) => {
      await addInstance(url, email, password, wsUrl, totpCode);
    },
  });
};

export const useCompleteRegistration = () => {
  const { addValidatedInstance } = useInstanceStore();

  return useMutation({
    mutationFn: async ({
      token,
      password,
      displayName,
    }: {
      token: string;
      password: string;
      displayName: string;
    }) => {
      const instanceUrl = window.location.origin;
      const clients = instanceManager.get(instanceUrl);
      clients.api.onSessionExpired = () => useInstanceStore.getState().removeInstance(instanceUrl);

      const res = await clients.api.typed((c) =>
        c.POST('/auth/complete-registration', { body: { token, password, display_name: displayName } }),
      );

      clients.api.setTokens(res.access_token, res.refresh_token);
      clients.ws.onStatusChange = (status) => {
        useWsStatusStore.getState().setStatus(instanceUrl, status);
      };
      clients.ws.connect();

      const config: InstanceConfig = { url: instanceUrl, user: toInstanceUser(res.user) };
      addValidatedInstance(config);

      return res;
    },
  });
};

export const useLogout = (instanceUrl?: string) => {
  const queryClient = useQueryClient();
  const { removeInstance, instances } = useInstanceStore();

  return useMutation({
    mutationFn: async () => {
      const url = instanceUrl ?? useInstanceStore.getState().activeInstanceUrl;
      if (url) {
        removeInstance(url);
      } else {
        for (const inst of [...instances]) {
          removeInstance(inst.url);
        }
      }
    },
    onSuccess: () => {
      queryClient.clear();
    },
  });
};
