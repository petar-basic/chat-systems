import { useMutation, useQuery } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { useCurrentApi } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';

export type AppDirectory = components['schemas']['AppDirectory'];
export type InstalledApp = components['schemas']['InstalledApp'];
export type InstalledKind = components['schemas']['InstalledKind'];
export type AvailableApp = components['schemas']['AvailableApp'];
export type AppPermission = components['schemas']['Permission'];
export type AppHealth = components['schemas']['Health'];

export interface AppRef {
  kind: InstalledKind;
  id: string;
}

export function sameApp(app: InstalledApp, ref: AppRef) {
  return app.kind === ref.kind && app.id === ref.id;
}

export function useAppDirectory(workspaceId: string | undefined) {
  const apiClient = useCurrentApi();
  return useQuery({
    queryKey: QUERY_KEYS.appDirectory(workspaceId ?? ''),
    enabled: !!workspaceId,
    queryFn: () =>
      apiClient.typed((c) =>
        c.GET('/workspaces/{ws_id}/app-directory', { params: { path: { ws_id: workspaceId ?? '' } } }),
      ),
  });
}

export function useRequestApp(workspaceId: string | undefined) {
  const apiClient = useCurrentApi();
  return useMutation({
    mutationFn: (request: { name: string; reason: string }) =>
      apiClient.typed((c) =>
        c.POST('/workspaces/{ws_id}/app-requests', {
          params: { path: { ws_id: workspaceId ?? '' } },
          body: { name: request.name, reason: request.reason || null },
        }),
      ),
  });
}
