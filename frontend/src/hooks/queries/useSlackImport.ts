import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';

export type SlackImportStatus = 'pending' | 'running' | 'complete' | 'failed';

export interface SlackImportReport {
  users_matched: number;
  users_created: number;
  channels_created: number;
  channels_reused: number;
  conversations_created: number;
  memberships: number;
  messages_imported: number;
  messages_already_present: number;
  threads_resolved: number;
  reactions: number;
  pins: number;
  files_imported: number;
  emoji_imported: number;
  skipped?: { what: string; why: string }[];
  notes?: string[];
}

export interface SlackImportRun {
  id: string;
  workspace_id: string;
  source: string;
  dry_run: boolean;
  status: SlackImportStatus;
  report: SlackImportReport | Record<string, never>;
  error: string | null;
  started_at: string;
  finished_at: string | null;
}

const isRunning = (runs: SlackImportRun[]) =>
  runs.some((run) => run.status === 'pending' || run.status === 'running');

export const useSlackImports = (workspaceId: string | null, instanceUrl?: string) => {
  return useQuery({
    queryKey: QUERY_KEYS.slackImports(workspaceId ?? ''),
    queryFn: async () => {
      const res = await getApiForInstance(instanceUrl).get<{ data: SlackImportRun[] }>(
        `/workspaces/${workspaceId}/slack-imports`,
      );
      return res.data;
    },
    enabled: !!workspaceId && !!instanceUrl,
    // An import writes its counters as it goes, so a run in flight is worth
    // asking about again; a finished one is not.
    refetchInterval: (query) => (isRunning(query.state.data ?? []) ? 2000 : false),
  });
};

export const useStartSlackImport = (workspaceId: string, instanceUrl?: string) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({
      archive,
      dryRun,
      newWorkspaceName,
    }: {
      archive: File;
      dryRun: boolean;
      newWorkspaceName?: string;
    }) => {
      const form = new FormData();
      form.append('dry_run', String(dryRun));
      form.append('archive', archive);
      if (newWorkspaceName) form.append('workspace_name', newWorkspaceName);

      // A whole Slack workspace has nothing to be imported *into* yet; the
      // export names no workspace, so the name travels with the upload.
      const path = newWorkspaceName ? '/slack-imports' : `/workspaces/${workspaceId}/slack-imports`;
      return getApiForInstance(instanceUrl).upload<{ data: SlackImportRun }>(path, form);
    },
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: QUERY_KEYS.slackImports(workspaceId) });
      await queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaces() });
    },
  });
};

/// `Acme Slack export Jul 27 2026 - Aug 26 2026.zip` is the only place the
/// workspace's name appears — Slack puts it in the file name and nowhere inside
/// the archive, not even in the manifest.
export function workspaceNameFrom(filename: string): string {
  const withoutExtension = filename.replace(/\.zip$/i, '');
  const beforeExport = withoutExtension.split(/\s+Slack export\s+/i)[0];
  return beforeExport.trim().slice(0, 100);
}
