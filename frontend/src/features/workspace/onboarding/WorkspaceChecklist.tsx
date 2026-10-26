import { useState, type ReactNode } from 'react';
import { CheckCircle2, Circle, ExternalLink, X } from 'lucide-react';
import { Button } from '@/shared/components/Button/Button';
import { IconButton } from '@/shared/components/IconButton/IconButton';
import { useWorkspaceInvites } from '@/hooks/queries/useWorkspaces';
import { useInstanceInfo } from '@/hooks/queries/useInstanceInfo';
import type { Channel, Workspace, WorkspaceMember } from '@/stores/workspace';
import {
  SSO_RUNBOOK_URL,
  checklistSteps,
  isGettingStarted,
  rememberDismissal,
  wasDismissed,
  type StepKey,
} from './checklist';

interface Props {
  workspace: Workspace;
  members: WorkspaceMember[];
  channels: Channel[];
  onInvite: () => void;
  onCreateChannel: () => void;
  onSetIcon: () => void;
}

export function WorkspaceChecklist({
  workspace,
  members,
  channels,
  onInvite,
  onCreateChannel,
  onSetIcon,
}: Props) {
  const [dismissed, setDismissed] = useState(() => wasDismissed(workspace.id));
  const gettingStarted = !dismissed && members.length > 0 && isGettingStarted(members.length, channels);
  const { data: invites = [] } = useWorkspaceInvites(workspace.id, workspace.instanceUrl, gettingStarted);
  const { data: info } = useInstanceInfo(workspace.instanceUrl);

  const steps = checklistSteps({
    memberCount: members.length,
    inviteCount: invites.length,
    channels,
    iconUrl: workspace.icon_url,
    ssoEnabled: !!info?.sso_enabled,
  });
  const done = steps.filter((s) => s.done).length;

  if (!gettingStarted || done === steps.length) return null;

  const dismiss = () => {
    rememberDismissal(workspace.id);
    setDismissed(true);
  };

  const actions: Record<StepKey, ReactNode> = {
    invite: (
      <Button size="sm" onClick={onInvite} data-qa="checklist-invite">
        Invite people
      </Button>
    ),
    channel: (
      <Button size="sm" onClick={onCreateChannel} data-qa="checklist-channel">
        Create
      </Button>
    ),
    icon: (
      <Button size="sm" onClick={onSetIcon} data-qa="checklist-icon">
        Choose
      </Button>
    ),
    sso: (
      <a
        href={SSO_RUNBOOK_URL}
        target="_blank"
        rel="noreferrer noopener"
        className="inline-flex items-center gap-1 text-xs text-accent hover:underline"
        data-qa="checklist-sso"
      >
        How to <ExternalLink className="w-3 h-3" />
      </a>
    ),
  };

  return (
    <section
      aria-labelledby="workspace-checklist-title"
      className="mx-4 mt-3 rounded-xl border border-line bg-surface/60 p-4 shrink-0"
      data-qa="workspace-checklist"
    >
      <div className="flex items-start gap-3">
        <div className="flex-1 min-w-0">
          <h2 id="workspace-checklist-title" className="text-sm font-semibold text-fg">
            Get {workspace.name} going
          </h2>
          <p className="text-xs text-muted" data-qa="checklist-progress">
            {done} of {steps.length} done
          </p>
        </div>
        <IconButton
          label="Dismiss the checklist"
          icon={<X className="w-4 h-4" />}
          onClick={dismiss}
          data-qa="checklist-dismiss"
        />
      </div>
      <ul className="mt-3 grid gap-2 sm:grid-cols-2">
        {steps.map((step) => (
          <li
            key={step.key}
            className="flex items-center gap-2 min-h-8"
            data-qa="checklist-step"
            data-step={step.key}
            data-done={step.done}
          >
            {step.done ? (
              <CheckCircle2 className="w-4 h-4 shrink-0 text-success" aria-hidden />
            ) : (
              <Circle className="w-4 h-4 shrink-0 text-faint" aria-hidden />
            )}
            <span className={`flex-1 text-sm ${step.done ? 'text-muted line-through' : 'text-fg'}`}>
              {step.label}
              {step.done && <span className="sr-only"> (done)</span>}
            </span>
            {!step.done && actions[step.key]}
          </li>
        ))}
      </ul>
    </section>
  );
}
