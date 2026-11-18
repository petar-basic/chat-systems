import { useMemo, useState } from 'react';
import { useMutation, useQuery } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { useCurrentApi } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';
import { useWorkspaceStore, type Message } from '@/stores/workspace';
import { actionKey, refusalText, type BlockActions } from './blockActions';

export type InteractiveApp = components['schemas']['InteractiveApp'];

export function useInteractiveApps(workspaceId: string | undefined) {
  const apiClient = useCurrentApi();
  return useQuery({
    queryKey: QUERY_KEYS.interactiveApps(workspaceId ?? ''),
    enabled: !!workspaceId,
    staleTime: 60_000,
    queryFn: () =>
      apiClient
        .typed((c) =>
          c.GET('/workspaces/{ws_id}/interactive-apps', {
            params: { path: { ws_id: workspaceId ?? '' } },
          }),
        )
        .then((list) => list.data),
  });
}

export function postedByInteractiveApp(apps: InteractiveApp[], message: Message): boolean {
  const hookId = message.metadata?.bot?.hook_id;
  return apps.some(
    (app) => app.bot_user_id === message.user_id || (!!hookId && app.hook_ids.includes(hookId)),
  );
}

interface Acting {
  blockId: string;
  actionId: string;
  value?: string;
}

export function useMessageActions(message: Message): BlockActions | null {
  const workspaceId = useWorkspaceStore((s) => s.currentWorkspace?.id);
  const apiClient = useCurrentApi();
  const { data: apps } = useInteractiveApps(workspaceId);
  const [failure, setFailure] = useState<BlockActions['failure']>(null);
  const live = !!apps && postedByInteractiveApp(apps, message);

  const { mutate, isPending, variables } = useMutation({
    retry: 0,
    mutationFn: ({ blockId, actionId, value }: Acting) =>
      apiClient.typed((c) =>
        c.POST('/messages/{msg_id}/actions', {
          params: { path: { msg_id: message.id } },
          body: { block_id: blockId, action_id: actionId, value },
        }),
      ),
    onMutate: () => setFailure(null),
    onError: (error, { blockId }) => setFailure({ blockId, text: refusalText(error) }),
  });

  const pending = isPending && variables ? actionKey(variables.blockId, variables.actionId) : null;

  return useMemo(
    () =>
      live
        ? {
            act: (blockId: string, actionId: string, value?: string) => mutate({ blockId, actionId, value }),
            pending,
            failure,
          }
        : null,
    [live, mutate, pending, failure],
  );
}
