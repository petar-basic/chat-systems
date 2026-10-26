import { useState } from 'react';
import { Plus, Workflow as WorkflowIcon } from 'lucide-react';
import { PanelHeader } from '@/shared/components/PanelHeader/PanelHeader';
import { Button } from '@/shared/components/Button/Button';
import { Spinner } from '@/shared/components/Spinner/Spinner';
import { formatDateTime } from '@/lib/datetime';
import { displayNameOf } from '@/lib/userHelpers';
import type { Channel, WorkspaceMember } from '@/stores/workspace';
import { WorkflowEditor } from './WorkflowEditor';
import { NewWorkflowDialog } from './NewWorkflowDialog';
import type { Starter } from './templates';
import { useWorkflows } from './useWorkflows';
import { triggerSummary, type Workflow } from './workflowModel';

interface Props {
  workspaceId: string;
  instanceUrl?: string;
  channels: Channel[];
  members: WorkspaceMember[];
  currentUserId: string;
  canBuild: boolean;
  workflowId?: string;
  onClose: () => void;
}

const LAST_RUN: Record<NonNullable<Workflow['last_run']>['status'], string> = {
  pending: 'Queued',
  running: 'Running',
  succeeded: 'Last run succeeded',
  failed: 'Last run failed',
  waiting: 'Waiting for a form',
  expired: 'Last run expired',
};

export default function WorkflowsPanel({
  workspaceId,
  instanceUrl,
  channels,
  members,
  currentUserId,
  canBuild,
  workflowId,
  onClose,
}: Props) {
  const workflows = useWorkflows(workspaceId, instanceUrl);
  const [editing, setEditing] = useState<Workflow | Starter | 'new' | null>(null);
  const [opening, setOpening] = useState(workflowId);
  const opened = opening ? workflows.data?.find((w) => w.id === opening) : undefined;
  if (opened) {
    setOpening(undefined);
    setEditing(opened);
  }
  const nameOf = (userId: string) => displayNameOf(members.find((m) => m.user_id === userId)?.display_name);
  const channelName = (channelId: string) => channels.find((c) => c.id === channelId)?.name ?? undefined;
  const postable = channels.filter(
    (c) => (c.channel_type === 'public' || c.channel_type === 'private') && !c.is_archived,
  );
  const starting = (postable.find((c) => c.name === 'general') ?? postable[0])?.id ?? '';

  return (
    <aside
      aria-label="Workflows"
      className="w-full lg:w-96 max-lg:fixed max-lg:inset-0 max-lg:z-40 bg-surface border-l border-line/50 flex flex-col"
      data-qa="workflows-panel"
    >
      <PanelHeader
        title="Workflows"
        icon={<WorkflowIcon className="h-4 w-4 text-muted" />}
        onClose={onClose}
        actions={
          canBuild && (
            <Button
              size="sm"
              variant="primary"
              icon={<Plus className="h-3.5 w-3.5" />}
              onClick={() => setEditing('new')}
              data-qa="workflow-new"
            >
              New
            </Button>
          )
        }
      />
      <div className="flex-1 overflow-y-auto">
        {workflows.isLoading ? (
          <div className="flex justify-center py-8 text-accent">
            <Spinner />
          </div>
        ) : (workflows.data ?? []).length === 0 ? (
          <p className="px-4 py-8 text-center text-sm text-muted" data-qa="workflows-empty">
            {canBuild
              ? 'No workflows yet. Post a standup reminder on a schedule, welcome people who join a channel, or turn a webhook call into a message.'
              : 'Only admins can build workflows in this workspace.'}
          </p>
        ) : (
          <ul>
            {(workflows.data ?? []).map((workflow) => (
              <li key={workflow.id} className="border-b border-line/40">
                <button
                  type="button"
                  onClick={() => setEditing(workflow)}
                  className="w-full cursor-pointer px-4 py-3 text-left transition hover:bg-raised/40"
                  data-qa="workflow-row"
                  data-workflow-id={workflow.id}
                >
                  <div className="flex items-center gap-2">
                    <span className="flex-1 truncate text-sm font-medium text-fg-soft">{workflow.name}</span>
                    <span
                      className={`rounded px-1.5 py-0.5 text-[10px] font-semibold ${
                        workflow.enabled ? 'bg-green-500/15 text-success' : 'bg-raised text-muted'
                      }`}
                      data-qa="workflow-state"
                    >
                      {workflow.enabled ? 'On' : 'Off'}
                    </span>
                  </div>
                  <div className="truncate text-xs text-muted" data-qa="workflow-trigger-summary">
                    {triggerSummary(workflow.definition.trigger, channelName)}
                  </div>
                  {workflow.last_run && (
                    <div
                      className={`truncate text-xs ${workflow.last_run.status === 'failed' ? 'text-danger' : 'text-muted'}`}
                      data-qa="workflow-last-run"
                    >
                      {LAST_RUN[workflow.last_run.status]} · {formatDateTime(workflow.last_run.started_at)}
                    </div>
                  )}
                  {workflow.created_by !== currentUserId && (
                    <div className="truncate text-xs text-subtle" data-qa="workflow-owner">
                      Built by {nameOf(workflow.created_by)}
                    </div>
                  )}
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
      {editing === 'new' && (
        <NewWorkflowDialog channelId={starting} onPick={setEditing} onClose={() => setEditing(null)} />
      )}
      {editing && editing !== 'new' && (
        <WorkflowEditor
          workspaceId={workspaceId}
          instanceUrl={instanceUrl}
          workflow={'id' in editing ? editing : null}
          starter={'id' in editing ? undefined : editing}
          channels={channels}
          members={members}
          currentUserId={currentUserId}
          canBuild={canBuild}
          onClose={() => setEditing(null)}
        />
      )}
    </aside>
  );
}
