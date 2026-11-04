import { useEffect, useEffectEvent } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';
import { useWorkspaceStore } from '@/stores/workspace';
import { useThemeStore } from '@/stores/theme';
import { useDisplayStore } from '@/stores/display';
import { applyLanguage } from '@/lib/i18n';

type Preferences = components['schemas']['Preferences'];
type PreferenceChanges = components['schemas']['UpdatePreferences'];

export const usePreferences = () => {
  const instanceUrl = useWorkspaceStore((s) => s.currentWorkspace?.instanceUrl);
  return useQuery({
    queryKey: QUERY_KEYS.preferences(instanceUrl ?? ''),
    queryFn: async (): Promise<Preferences> =>
      getApiForInstance(instanceUrl).typed((c) => c.GET('/users/me/preferences')),
    enabled: !!instanceUrl,
  });
};

export const useUpdatePreferences = () => {
  const queryClient = useQueryClient();
  const instanceUrl = useWorkspaceStore((s) => s.currentWorkspace?.instanceUrl);
  const key = QUERY_KEYS.preferences(instanceUrl ?? '');
  return useMutation({
    mutationFn: (changes: PreferenceChanges) =>
      getApiForInstance(instanceUrl).typed((c) => c.PATCH('/users/me/preferences', { body: changes })),
    onMutate: async (changes) => {
      await queryClient.cancelQueries({ queryKey: key });
      const before = queryClient.getQueryData<Preferences>(key);
      const changed: Partial<Preferences> = Object.fromEntries(
        Object.entries(changes).filter(([, value]) => value != null),
      );
      if (before) queryClient.setQueryData<Preferences>(key, { ...before, ...changed });
      return { before };
    },
    onError: (_error, _changes, context) => {
      if (context?.before) queryClient.setQueryData(key, context.before);
    },
    onSuccess: (saved) => queryClient.setQueryData(key, saved),
  });
};

export function usePreferencesSync() {
  const { data } = usePreferences();
  const update = useUpdatePreferences();
  const show = useDisplayStore((s) => s.show);

  useEffect(() => {
    if (data)
      show({
        density: data.density,
        previews: data.previews,
        enterKey: data.enter_key,
        openChannelAt: data.open_channel_at,
      });
  }, [data, show]);

  const followColourMode = useEffectEvent((saved: Preferences['colour_mode'] | undefined) => {
    const { mode, setMode } = useThemeStore.getState();
    if (saved == null) update.mutate({ colour_mode: mode });
    else if (saved !== mode) setMode(saved);
  });
  const loaded = data !== undefined;
  const savedMode = data?.colour_mode;
  useEffect(() => {
    if (loaded) followColourMode(savedMode);
  }, [loaded, savedMode]);

  const savedLanguage = data?.language;
  useEffect(() => {
    if (loaded) void applyLanguage(savedLanguage);
  }, [loaded, savedLanguage]);
}
