import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { useCurrentApi } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS, ErrorLabels } from '@/shared/constants';
import { toast } from '@/shared/components/Toast';
import { toUserMessage } from '@/lib/errors';
import { logger } from '@/lib/logger';

export type SidebarLayout = components['schemas']['SidebarLayout'];
export type SidebarSectionData = components['schemas']['SidebarSection'];
export type BuiltinSection = components['schemas']['BuiltinSection'];

export function useSidebarLayout(workspaceId: string | undefined) {
  const apiClient = useCurrentApi();
  return useQuery({
    queryKey: QUERY_KEYS.sidebar(workspaceId ?? ''),
    enabled: !!workspaceId,
    queryFn: () =>
      apiClient.typed((c) =>
        c.GET('/workspaces/{ws_id}/sidebar', { params: { path: { ws_id: workspaceId ?? '' } } }),
      ),
  });
}

export function useSidebarActions(workspaceId: string | undefined) {
  const apiClient = useCurrentApi();
  const queryClient = useQueryClient();
  const key = QUERY_KEYS.sidebar(workspaceId ?? '');
  const ws = workspaceId ?? '';

  const patch = (update: (layout: SidebarLayout) => SidebarLayout) =>
    queryClient.setQueryData<SidebarLayout>(key, (old) => (old ? update(old) : old));

  const failed = (where: string) => (err: unknown) => {
    logger.error('useSidebarActions', where, err);
    toast.error(toUserMessage(err, ErrorLabels.SidebarSaveFailed));
    void queryClient.invalidateQueries({ queryKey: key });
  };

  const createSection = useMutation({
    mutationFn: (name: string) =>
      apiClient.typed((c) =>
        c.POST('/workspaces/{ws_id}/sidebar/sections', { params: { path: { ws_id: ws } }, body: { name } }),
      ),
    onSuccess: (section) => patch((l) => ({ ...l, sections: [...l.sections, section] })),
    onError: failed('createSection'),
  });

  const updateSection = useMutation({
    mutationFn: ({ id, name, collapsed }: { id: string; name?: string; collapsed?: boolean }) =>
      apiClient.typed((c) =>
        c.PATCH('/workspaces/{ws_id}/sidebar/sections/{section_id}', {
          params: { path: { ws_id: ws, section_id: id } },
          body: { name, collapsed },
        }),
      ),
    onMutate: ({ id, name, collapsed }) =>
      patch((l) => ({
        ...l,
        sections: l.sections.map((s) =>
          s.id === id ? { ...s, name: name ?? s.name, collapsed: collapsed ?? s.collapsed } : s,
        ),
      })),
    onError: failed('updateSection'),
  });

  const deleteSection = useMutation({
    mutationFn: (id: string) =>
      apiClient.typed((c) =>
        c.DELETE('/workspaces/{ws_id}/sidebar/sections/{section_id}', {
          params: { path: { ws_id: ws, section_id: id } },
        }),
      ),
    onMutate: (id) =>
      patch((l) => ({
        ...l,
        sections: l.sections.filter((s) => s.id !== id),
        placements: l.placements.filter((p) => p.section_id !== id),
      })),
    onError: failed('deleteSection'),
  });

  const reorderSections = useMutation({
    mutationFn: (sectionIds: string[]) =>
      apiClient.typed((c) =>
        c.PUT('/workspaces/{ws_id}/sidebar/order', {
          params: { path: { ws_id: ws } },
          body: { section_ids: sectionIds },
        }),
      ),
    onMutate: (sectionIds) =>
      patch((l) => ({
        ...l,
        sections: sectionIds
          .map((id, position) => {
            const section = l.sections.find((s) => s.id === id);
            return section ? { ...section, position } : null;
          })
          .filter((s): s is SidebarSectionData => s !== null),
      })),
    onError: failed('reorderSections'),
  });

  const placeChannel = useMutation({
    mutationFn: ({ channelId, sectionId }: { channelId: string; sectionId: string | null }) =>
      apiClient.typed((c) =>
        c.PUT('/channels/{ch_id}/section', {
          params: { path: { ch_id: channelId } },
          body: { section_id: sectionId },
        }),
      ),
    onMutate: ({ channelId, sectionId }) =>
      patch((l) => ({
        ...l,
        placements: [
          ...l.placements.filter((p) => p.channel_id !== channelId),
          ...(sectionId ? [{ channel_id: channelId, section_id: sectionId }] : []),
        ],
      })),
    onError: failed('placeChannel'),
  });

  const collapseBuiltin = useMutation({
    mutationFn: ({ section, collapsed }: { section: BuiltinSection; collapsed: boolean }) =>
      apiClient.typed((c) =>
        c.PUT('/workspaces/{ws_id}/sidebar/builtins', {
          params: { path: { ws_id: ws } },
          body: { section, collapsed },
        }),
      ),
    onMutate: ({ section, collapsed }) =>
      patch((l) => ({
        ...l,
        collapsed_builtins: collapsed
          ? [...l.collapsed_builtins.filter((s) => s !== section), section]
          : l.collapsed_builtins.filter((s) => s !== section),
      })),
    onError: failed('collapseBuiltin'),
  });

  return { createSection, updateSection, deleteSection, reorderSections, placeChannel, collapseBuiltin };
}
