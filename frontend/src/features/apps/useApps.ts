import { useMemo } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { useCurrentApi } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';
import { fromChannelContext } from '@/features/channel';

export type ActiveApp = components['schemas']['ActiveApp'];
export type AppShortcut = components['schemas']['AppShortcut'];
export type AppHome = components['schemas']['AppHome'];
export type HomeTab = components['schemas']['HomeTab'];

export interface GlobalShortcut {
  shortcut: AppShortcut;
  app: ActiveApp;
}

export function useActiveApps(workspaceId: string | undefined) {
  const apiClient = useCurrentApi();
  return useQuery({
    queryKey: QUERY_KEYS.activeApps(workspaceId ?? ''),
    enabled: !!workspaceId,
    staleTime: 60_000,
    queryFn: () =>
      apiClient
        .typed((c) =>
          c.GET('/workspaces/{ws_id}/active-apps', {
            params: { path: { ws_id: workspaceId ?? '' } },
          }),
        )
        .then((list) => list.data),
  });
}

export function useAppOfUser(workspaceId: string | undefined, userId: string | undefined) {
  const { data } = useActiveApps(workspaceId);
  return userId ? data?.find((app) => app.bot_user_id === userId) : undefined;
}

export function useGlobalShortcuts(workspaceId: string | undefined): GlobalShortcut[] {
  const { data } = useActiveApps(workspaceId);
  return useMemo(
    () => (data ?? []).flatMap((app) => app.shortcuts.map((shortcut) => ({ shortcut, app }))),
    [data],
  );
}

export function useMessageShortcuts(channelId: string | undefined) {
  const queryClient = useQueryClient();
  const apiClient = useCurrentApi();
  return useQuery({
    queryKey: QUERY_KEYS.messageShortcuts(channelId ?? ''),
    enabled: !!channelId,
    staleTime: 30_000,
    queryFn: () =>
      fromChannelContext(
        queryClient,
        apiClient,
        channelId ?? '',
        (context) => context.message_shortcuts,
        async () => {
          const res = await apiClient.typed((c) =>
            c.GET('/channels/{ch_id}/message-shortcuts', {
              params: { path: { ch_id: channelId ?? '' } },
            }),
          );
          return res.data;
        },
      ),
  });
}

export interface ShortcutUse {
  shortcutId: string;
  channelId?: string;
  messageId?: string;
}

export function useRunAppShortcut() {
  const apiClient = useCurrentApi();
  return useMutation({
    retry: 0,
    mutationFn: ({ shortcutId, channelId, messageId }: ShortcutUse) =>
      apiClient.typed((c) =>
        c.POST('/app-shortcuts/{shortcut_id}/run', {
          params: { path: { shortcut_id: shortcutId } },
          body: { channel_id: channelId, message_id: messageId },
        }),
      ),
  });
}

export function useAppHome(appId: string) {
  const apiClient = useCurrentApi();
  return useQuery({
    queryKey: QUERY_KEYS.appHome(appId),
    staleTime: 60_000,
    queryFn: () =>
      apiClient.typed((c) => c.GET('/apps/{app_id}/home', { params: { path: { app_id: appId } } })),
  });
}

export function useAppHomeOpened(appId: string) {
  const apiClient = useCurrentApi();
  return useMutation({
    retry: 0,
    mutationFn: (channelId: string) =>
      apiClient.typed((c) =>
        c.POST('/apps/{app_id}/home/opened', {
          params: { path: { app_id: appId } },
          body: { channel_id: channelId },
        }),
      ),
  });
}

export interface HomeAction {
  blockId: string;
  actionId: string;
  value?: string;
}

export function useAppHomeAction(appId: string) {
  const apiClient = useCurrentApi();
  return useMutation({
    retry: 0,
    mutationFn: ({ blockId, actionId, value }: HomeAction) =>
      apiClient.typed((c) =>
        c.POST('/apps/{app_id}/home/actions', {
          params: { path: { app_id: appId } },
          body: { block_id: blockId, action_id: actionId, value },
        }),
      ),
  });
}

export function useBotAppShortcuts(appId: string) {
  const apiClient = useCurrentApi();
  return useQuery({
    queryKey: QUERY_KEYS.botAppShortcuts(appId),
    queryFn: () =>
      apiClient
        .typed((c) => c.GET('/bot-apps/{app_id}/shortcuts', { params: { path: { app_id: appId } } }))
        .then((list) => list.data),
  });
}

export type ShortcutDraft = components['schemas']['AppShortcutInput'];

export function useSaveBotAppShortcuts(appId: string) {
  const apiClient = useCurrentApi();
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (shortcuts: ShortcutDraft[]) =>
      apiClient
        .typed((c) =>
          c.PUT('/bot-apps/{app_id}/shortcuts', {
            params: { path: { app_id: appId } },
            body: { shortcuts },
          }),
        )
        .then((list) => list.data),
    onSuccess: (saved) => queryClient.setQueryData(QUERY_KEYS.botAppShortcuts(appId), saved),
  });
}

export type WorkflowStepDraft = components['schemas']['WorkflowStepInput'];

export function useBotAppWorkflowSteps(appId: string) {
  const apiClient = useCurrentApi();
  return useQuery({
    queryKey: QUERY_KEYS.botAppWorkflowSteps(appId),
    queryFn: () =>
      apiClient
        .typed((c) => c.GET('/bot-apps/{app_id}/workflow-steps', { params: { path: { app_id: appId } } }))
        .then((listed) => listed.steps),
  });
}

export function useSaveBotAppWorkflowSteps(appId: string) {
  const apiClient = useCurrentApi();
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (steps: WorkflowStepDraft[]) =>
      apiClient
        .typed((c) =>
          c.PUT('/bot-apps/{app_id}/workflow-steps', {
            params: { path: { app_id: appId } },
            body: { steps },
          }),
        )
        .then((saved) => saved.steps),
    onSuccess: (saved) => queryClient.setQueryData(QUERY_KEYS.botAppWorkflowSteps(appId), saved),
  });
}

export function useBotAppUnfurlDomains(appId: string) {
  const apiClient = useCurrentApi();
  return useQuery({
    queryKey: QUERY_KEYS.botAppUnfurlDomains(appId),
    queryFn: () =>
      apiClient
        .typed((c) => c.GET('/bot-apps/{app_id}/unfurl-domains', { params: { path: { app_id: appId } } }))
        .then((saved) => saved.domains),
  });
}

export function useSaveBotAppUnfurlDomains(appId: string) {
  const apiClient = useCurrentApi();
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (domains: string[]) =>
      apiClient
        .typed((c) =>
          c.PUT('/bot-apps/{app_id}/unfurl-domains', {
            params: { path: { app_id: appId } },
            body: { domains },
          }),
        )
        .then((saved) => saved.domains),
    onSuccess: (saved) => queryClient.setQueryData(QUERY_KEYS.botAppUnfurlDomains(appId), saved),
  });
}
