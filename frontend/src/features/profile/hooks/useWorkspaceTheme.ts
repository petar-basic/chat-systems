import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';
import { useWorkspaceStore } from '@/stores/workspace';
import type { Theme } from '../workspaceThemes';

type WorkspaceTheme = components['schemas']['WorkspaceTheme'];

const useWorkspaceThemes = (instanceUrl: string | undefined) =>
  useQuery({
    queryKey: QUERY_KEYS.workspaceThemes(instanceUrl ?? ''),
    queryFn: async (): Promise<WorkspaceTheme[]> =>
      (await getApiForInstance(instanceUrl).typed((c) => c.GET('/users/me/themes'))).data,
  });

export const useWorkspaceTheme = (): Theme | null => {
  const workspace = useWorkspaceStore((s) => s.currentWorkspace);
  const { data } = useWorkspaceThemes(workspace?.instanceUrl);
  if (!workspace) return null;
  return data?.find((entry) => entry.workspace_id === workspace.id)?.theme ?? null;
};

export const useSetWorkspaceTheme = () => {
  const queryClient = useQueryClient();
  const workspace = useWorkspaceStore((s) => s.currentWorkspace);
  const key = QUERY_KEYS.workspaceThemes(workspace?.instanceUrl ?? '');
  return useMutation({
    mutationFn: (theme: Theme | null) =>
      getApiForInstance(workspace?.instanceUrl).typed((c) =>
        c.PUT('/workspaces/{ws_id}/profile/theme', {
          params: { path: { ws_id: workspace?.id ?? '' } },
          body: { theme },
        }),
      ),
    onMutate: async (theme) => {
      await queryClient.cancelQueries({ queryKey: key });
      const before = queryClient.getQueryData<WorkspaceTheme[]>(key);
      const others = (before ?? []).filter((entry) => entry.workspace_id !== workspace?.id);
      queryClient.setQueryData<WorkspaceTheme[]>(
        key,
        theme && workspace ? [...others, { workspace_id: workspace.id, theme }] : others,
      );
      return { before };
    },
    onError: (_error, _theme, context) => {
      queryClient.setQueryData(key, context?.before);
    },
    onSettled: () => queryClient.invalidateQueries({ queryKey: key }),
  });
};
