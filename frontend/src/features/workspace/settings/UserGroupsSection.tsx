import { useState, type FormEvent } from 'react';
import { Spinner } from '@/shared/components/Spinner/Spinner';
import { Button } from '@/shared/components/Button/Button';
import { Trash2, Plus, Check } from 'lucide-react';
import {
  useUserGroups,
  useCreateGroup,
  useDeleteGroup,
  useSetGroupMember,
} from '@/features/workspace/hooks/useUserGroups';
import type { WorkspaceMember } from '@/stores/workspace';
import { toUserMessage } from '@/lib/errors';
import { toast } from '@/shared/components/Toast';
import { displayNameOf } from '@/lib/userHelpers';
import { TextInput } from '@/shared/components/Form/TextInput';

interface Props {
  workspaceId: string;
  instanceUrl?: string;
  members: WorkspaceMember[];
  isAdmin: boolean;
}

export function UserGroupsSection({ workspaceId, instanceUrl, members, isAdmin }: Props) {
  const { data: groups, isLoading } = useUserGroups(workspaceId, instanceUrl);
  const createGroup = useCreateGroup(workspaceId, instanceUrl);
  const deleteGroup = useDeleteGroup(workspaceId, instanceUrl);
  const setMember = useSetGroupMember(workspaceId, instanceUrl);

  const [handle, setHandle] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [expanded, setExpanded] = useState<string | null>(null);
  const [groupMembers, setGroupMembers] = useState<Record<string, string[]>>({});

  const handleCreate = async (e: FormEvent) => {
    e.preventDefault();
    if (!handle.trim()) return;
    setError(null);
    try {
      await createGroup.mutateAsync({ handle: handle.trim() });
      setHandle('');
    } catch (err) {
      setError(toUserMessage(err));
    }
  };

  const openMembers = async (groupId: string) => {
    if (expanded === groupId) {
      setExpanded(null);
      return;
    }
    setExpanded(groupId);
    if (groupMembers[groupId]) return;
    try {
      const res = await fetch(
        `${instanceUrl ?? window.location.origin}/api/workspaces/${workspaceId}/groups/${groupId}/members`,
        { credentials: 'include' },
      );
      const body = await res.json();
      setGroupMembers((current) => ({ ...current, [groupId]: body.data ?? [] }));
    } catch {
      setGroupMembers((current) => ({ ...current, [groupId]: [] }));
    }
  };

  const toggleMember = async (groupId: string, userId: string, member: boolean) => {
    try {
      await setMember.mutateAsync({ groupId, userId, member });
      setGroupMembers((current) => {
        const existing = current[groupId] ?? [];
        return {
          ...current,
          [groupId]: member ? [...existing, userId] : existing.filter((id) => id !== userId),
        };
      });
    } catch (err) {
      toast.error(toUserMessage(err));
    }
  };

  return (
    <div className="-mx-4 flex flex-col">
      {isAdmin && (
        <form onSubmit={handleCreate} className="px-3 py-3 border-b border-line/30 space-y-2">
          <div className="flex gap-2">
            <TextInput
              label="Group handle"
              labelHidden
              value={handle}
              onChange={(e) => setHandle(e.target.value)}
              placeholder="handle (used as @handle)"
              dataQa="group-handle"
              className="flex-1"
            />
            <Button
              variant="primary"
              type="submit"
              disabled={createGroup.isPending || !handle.trim()}
              data-qa="group-create"
            >
              <Plus className="w-4 h-4" />
            </Button>
          </div>
          {error && <div className="text-xs text-danger">{error}</div>}
        </form>
      )}

      <div className="flex-1 overflow-y-auto px-2 py-2">
        {isLoading ? (
          <div className="flex justify-center py-8">
            <Spinner className="text-accent" />
          </div>
        ) : groups && groups.length > 0 ? (
          <ul className="space-y-1">
            {groups.map((group) => (
              <li key={group.id} data-qa="group-row" className="rounded-lg">
                <div className="flex items-center gap-2 px-3 py-2 hover:bg-raised/30 rounded-lg">
                  <button
                    type="button"
                    onClick={() => openMembers(group.id)}
                    className="flex-1 text-left cursor-pointer"
                  >
                    <span className="text-sm text-fg-soft">@{group.handle}</span>
                    <span className="ml-2 text-xs text-subtle">
                      {group.member_count} {group.member_count === 1 ? 'person' : 'people'}
                    </span>
                  </button>
                  {isAdmin && (
                    <button
                      type="button"
                      onClick={() => deleteGroup.mutate(group.id)}
                      aria-label={`Delete @${group.handle}`}
                      className="text-muted hover:text-danger transition cursor-pointer"
                    >
                      <Trash2 className="w-4 h-4" />
                    </button>
                  )}
                </div>

                {expanded === group.id && (
                  <ul className="pl-3 pb-2 space-y-0.5">
                    {members.map((member) => {
                      const inGroup = (groupMembers[group.id] ?? []).includes(member.user_id);
                      return (
                        <li key={member.user_id}>
                          <button
                            type="button"
                            disabled={!isAdmin}
                            onClick={() => toggleMember(group.id, member.user_id, !inGroup)}
                            className="w-full flex items-center gap-2 px-3 py-1.5 text-left text-sm text-fg-dim hover:bg-raised/30 rounded-lg transition cursor-pointer disabled:cursor-default"
                          >
                            <span className="w-4">
                              {inGroup && <Check className="w-3.5 h-3.5 text-success" />}
                            </span>
                            <span className="truncate">
                              {displayNameOf(member.display_name) || member.email}
                            </span>
                          </button>
                        </li>
                      );
                    })}
                  </ul>
                )}
              </li>
            ))}
          </ul>
        ) : (
          <p className="text-center py-8 text-muted text-sm">
            No groups yet. A group gives a team one handle to mention.
          </p>
        )}
      </div>
    </div>
  );
}
