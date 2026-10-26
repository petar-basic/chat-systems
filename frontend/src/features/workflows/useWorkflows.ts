import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';
import { fromChannelContext } from '@/features/channel/hooks/channelContext';
import type { SaveWorkflow, Workflow } from './workflowModel';

export const useWorkflowShortcuts = (channelId: string | null, instanceUrl?: string) => {
  const queryClient = useQueryClient();
  return useQuery({
    queryKey: QUERY_KEYS.workflowShortcuts(channelId ?? ''),
    queryFn: async () => {
      if (!channelId) throw new Error('No channel selected');
      const api = getApiForInstance(instanceUrl);
      return fromChannelContext(
        queryClient,
        api,
        channelId,
        (context) => context.workflow_shortcuts,
        async () => {
          const res = await api.typed((c) =>
            c.GET('/channels/{ch_id}/workflow-shortcuts', { params: { path: { ch_id: channelId } } }),
          );
          return res.data;
        },
      );
    },
    enabled: !!channelId,
    staleTime: 1000 * 30,
  });
};

export const useStartWorkflow = (instanceUrl?: string) =>
  useMutation({
    mutationFn: (workflowId: string) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.POST('/workflows/{workflow_id}/start', {
          params: { path: { workflow_id: workflowId } },
          headers: { 'idempotency-key': crypto.randomUUID() },
        }),
      ),
  });

export const useSubmitForm = (instanceUrl?: string) =>
  useMutation({
    mutationFn: ({
      runId,
      position,
      answers,
    }: {
      runId: string;
      position: number;
      answers: Record<string, string>;
    }) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.POST('/workflow-runs/{run_id}/forms/{position}', {
          params: { path: { run_id: runId, position } },
          body: { answers },
        }),
      ),
  });

export const useWorkflows = (workspaceId: string, instanceUrl?: string) =>
  useQuery({
    queryKey: QUERY_KEYS.workflows(workspaceId),
    queryFn: async () => {
      const res = await getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/workspaces/{ws_id}/workflows', { params: { path: { ws_id: workspaceId } } }),
      );
      return res.data;
    },
    staleTime: 1000 * 30,
  });

export const useWorkflowRuns = (workflowId: string | null, instanceUrl?: string) =>
  useQuery({
    queryKey: QUERY_KEYS.workflowRuns(workflowId ?? ''),
    queryFn: async () => {
      if (!workflowId) throw new Error('No workflow selected');
      const res = await getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/workflows/{workflow_id}/runs', { params: { path: { workflow_id: workflowId } } }),
      );
      return res.data;
    },
    enabled: !!workflowId,
    refetchInterval: 10_000,
  });

export function useWorkflowActions(workspaceId: string, instanceUrl?: string) {
  const queryClient = useQueryClient();
  const api = () => getApiForInstance(instanceUrl);
  const refresh = (workflow?: Workflow) => {
    if (workflow) {
      queryClient.setQueryData<Workflow[]>(QUERY_KEYS.workflows(workspaceId), (all) =>
        all?.some((w) => w.id === workflow.id)
          ? all.map((w) => (w.id === workflow.id ? workflow : w))
          : [...(all ?? []), workflow],
      );
    }
    return queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workflows(workspaceId) });
  };

  const create = useMutation({
    mutationFn: (body: SaveWorkflow) =>
      api().typed((c) =>
        c.POST('/workspaces/{ws_id}/workflows', { params: { path: { ws_id: workspaceId } }, body }),
      ),
    onSuccess: refresh,
  });
  const update = useMutation({
    mutationFn: ({ id, body }: { id: string; body: SaveWorkflow }) =>
      api().typed((c) => c.PUT('/workflows/{workflow_id}', { params: { path: { workflow_id: id } }, body })),
    onSuccess: refresh,
  });
  const enable = useMutation({
    mutationFn: (id: string) =>
      api().typed((c) =>
        c.POST('/workflows/{workflow_id}/enable', { params: { path: { workflow_id: id } } }),
      ),
    onSuccess: refresh,
  });
  const disable = useMutation({
    mutationFn: (id: string) =>
      api().typed((c) =>
        c.POST('/workflows/{workflow_id}/disable', { params: { path: { workflow_id: id } } }),
      ),
    onSuccess: refresh,
  });
  const remove = useMutation({
    mutationFn: (id: string) =>
      api().typed((c) => c.DELETE('/workflows/{workflow_id}', { params: { path: { workflow_id: id } } })),
    onSuccess: () => refresh(),
  });

  return { create, update, enable, disable, remove };
}
