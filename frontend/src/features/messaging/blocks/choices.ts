import { useMemo } from 'react';
import type { ComboboxOption } from '@/shared/components/Form/Combobox';
import { useWorkspaceChannels, useWorkspaceMembers } from '@/features/workspace';
import { displayNameOf } from '@/lib/userHelpers';

export function usePersonChoices(workspaceId: string | null, instanceUrl?: string): ComboboxOption[] {
  const { data: members = [] } = useWorkspaceMembers(workspaceId, instanceUrl);
  return useMemo(
    () =>
      members
        .filter((member) => !member.is_bot)
        .map((member) => ({
          value: member.user_id,
          label: displayNameOf(member.display_name),
          detail: member.title ?? undefined,
        })),
    [members],
  );
}

export function useChannelChoices(workspaceId: string | null, instanceUrl?: string): ComboboxOption[] {
  const { data: channels = [] } = useWorkspaceChannels(workspaceId, instanceUrl);
  return useMemo(
    () =>
      channels
        .filter((channel) => !channel.is_archived && channel.name)
        .map((channel) => ({ value: channel.id, label: `#${channel.name}` })),
    [channels],
  );
}
