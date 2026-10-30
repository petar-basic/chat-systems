import { useQuery } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';

export type SlashCommand = components['schemas']['CommandInfo'];

export function useSlashCommands(workspaceId: string | undefined, instanceUrl?: string, enabled = true) {
  return useQuery({
    queryKey: QUERY_KEYS.slashCommands(workspaceId ?? ''),
    enabled: enabled && Boolean(workspaceId),
    staleTime: 5 * 60 * 1000,
    queryFn: async () => {
      if (!workspaceId) throw new Error('No workspace selected');
      const res = await getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/workspaces/{ws_id}/commands', { params: { path: { ws_id: workspaceId } } }),
      );
      return res.data;
    },
  });
}

export function matchSlashCommands(commands: SlashCommand[], query: string): SlashCommand[] {
  const q = query.toLowerCase();
  return commands.filter((c) => c.command.toLowerCase().startsWith(q)).slice(0, 8);
}
