import { useState, type FormEvent } from 'react';
import { History, PenLine, Plus, Power, Trash2, UserCheck } from 'lucide-react';
import { Modal } from '@/shared/components/Modal/Modal';
import { Button } from '@/shared/components/Button/Button';
import { TextInput } from '@/shared/components/Form/TextInput';
import { TextArea } from '@/shared/components/Form/TextArea';
import { ConfirmDialog } from '@/shared/components/ConfirmDialog/ConfirmDialog';
import { toUserMessage } from '@/lib/errors';
import type { Channel, WorkspaceMember } from '@/stores/workspace';
import { TriggerCard } from './TriggerCard';
import { StepCard } from './StepCard';
import { RunHistory } from './RunHistory';
import { useWorkflowActions } from './useWorkflows';
import type { Starter } from './templates';
import {
  MAX_STEPS,
  NAME_MAX,
  blankDefinition,
  blankStep,
  cleanDefinition,
  formsBefore,
  moveStep,
  removeStep,
  stepVariables,
  triggerPerson,
  unpicked,
  type Definition,
  type SaveWorkflow,
  type Workflow,
} from './workflowModel';

interface Props {
  workspaceId: string;
  instanceUrl?: string;
  workflow: Workflow | null;
  starter?: Starter;
  channels: Channel[];
  members: WorkspaceMember[];
  currentUserId: string;
  canBuild: boolean;
  onClose: () => void;
}

export function WorkflowEditor({
  workspaceId,
  instanceUrl,
  workflow: initial,
  starter,
  channels,
  members,
  currentUserId,
  canBuild,
  onClose,
}: Props) {
  const postable = channels.filter(
    (c) => (c.channel_type === 'public' || c.channel_type === 'private') && !c.is_archived,
  );
  const [workflow, setWorkflow] = useState<Workflow | null>(initial);
  const [name, setName] = useState(initial?.name ?? starter?.name ?? '');
  const [description, setDescription] = useState(initial?.description ?? starter?.description ?? '');
  const [definition, setDefinition] = useState<Definition>(
    initial?.definition ?? starter?.definition ?? blankDefinition(postable[0]?.id ?? ''),
  );
  const [view, setView] = useState<'edit' | 'history'>('edit');
  const [error, setError] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const actions = useWorkflowActions(workspaceId, instanceUrl);

  const mine = !workflow || workflow.created_by === currentUserId;
  const readOnly = !mine || !canBuild;
  const people = members.filter((m) => m.user_id !== currentUserId);
  const busy =
    actions.create.isPending ||
    actions.update.isPending ||
    actions.enable.isPending ||
    actions.disable.isPending ||
    actions.takeOver.isPending ||
    actions.rotate.isPending ||
    actions.remove.isPending;

  const body = (): SaveWorkflow => ({
    name: name.trim(),
    description: description.trim() || null,
    definition: cleanDefinition(definition),
  });

  const save = async (): Promise<Workflow> => {
    const missing = unpicked(definition);
    if (missing) throw new Error(missing);
    const saved = workflow
      ? await actions.update.mutateAsync({ id: workflow.id, body: body() })
      : await actions.create.mutateAsync(body());
    setWorkflow(saved);
    setDefinition(saved.definition);
    return saved;
  };

  const run = async (action: () => Promise<unknown>, fallback: string) => {
    setError(null);
    try {
      await action();
    } catch (err) {
      setError(toUserMessage(err, fallback));
    }
  };

  const handleSave = (e: FormEvent) => {
    e.preventDefault();
    void run(save, "Couldn't save the workflow");
  };

  const handleEnable = () =>
    run(async () => {
      if (workflow) setWorkflow(await actions.enable.mutateAsync(workflow.id));
    }, "Couldn't turn the workflow on");

  const handleDisable = () =>
    run(async () => {
      if (workflow) setWorkflow(await actions.disable.mutateAsync(workflow.id));
    }, "Couldn't turn the workflow off");

  const handleTakeOver = () =>
    run(async () => {
      if (workflow) setWorkflow(await actions.takeOver.mutateAsync(workflow.id));
    }, "Couldn't take the workflow over");

  const handleRotate = () =>
    run(async () => {
      if (workflow) setWorkflow(await actions.rotate.mutateAsync(workflow.id));
    }, "Couldn't make a new URL");

  const setStep = (index: number, step: Definition['steps'][number]) =>
    setDefinition((d) => ({ ...d, steps: d.steps.map((s, i) => (i === index ? step : s)) }));

  return (
    <Modal
      title={workflow ? workflow.name : 'New workflow'}
      description={
        workflow
          ? workflow.enabled
            ? 'On: it runs whenever its trigger fires.'
            : 'Off: it won’t run until you turn it on.'
          : 'Pick what starts it, then what it does. It runs as soon as you create it.'
      }
      size="full"
      onClose={onClose}
      onSubmit={readOnly ? undefined : handleSave}
      dataQa="workflow-editor"
      headerActions={
        workflow && (
          <Button
            size="sm"
            variant="ghost"
            icon={view === 'edit' ? <History className="h-3.5 w-3.5" /> : <PenLine className="h-3.5 w-3.5" />}
            onClick={() => setView(view === 'edit' ? 'history' : 'edit')}
            data-qa="workflow-history-toggle"
          >
            {view === 'edit' ? 'Run history' : 'Back to the workflow'}
          </Button>
        )
      }
      footerStart={
        workflow && (
          <Button
            variant="ghost"
            icon={<Trash2 className="h-4 w-4" />}
            onClick={() => setConfirmDelete(true)}
            disabled={busy}
            data-qa="workflow-delete"
          >
            Delete
          </Button>
        )
      }
      dismissLabel="Close"
      actions={
        <>
          {workflow?.enabled && (
            <Button
              icon={<Power className="h-4 w-4" />}
              onClick={handleDisable}
              loading={actions.disable.isPending}
              disabled={busy}
              data-qa="workflow-disable"
            >
              Turn off
            </Button>
          )}
          {workflow && !mine && workflow.creator_gone && (
            <Button
              icon={<UserCheck className="h-4 w-4" />}
              onClick={handleTakeOver}
              loading={actions.takeOver.isPending}
              disabled={busy}
              data-qa="workflow-take-over"
            >
              Take over
            </Button>
          )}
          {workflow && !workflow.enabled && !readOnly && (
            <Button
              icon={<Power className="h-4 w-4" />}
              onClick={handleEnable}
              loading={actions.enable.isPending}
              disabled={busy}
              data-qa="workflow-enable"
            >
              Turn on
            </Button>
          )}
          {!readOnly && (
            <Button
              type="submit"
              variant="primary"
              loading={actions.create.isPending || actions.update.isPending}
              disabled={busy}
              data-qa="workflow-save"
            >
              {workflow ? 'Save changes' : 'Create workflow'}
            </Button>
          )}
        </>
      }
    >
      {view === 'history' && workflow ? (
        <RunHistory workflowId={workflow.id} steps={definition.steps} instanceUrl={instanceUrl} />
      ) : (
        <div className="space-y-4 py-3">
          <div className="grid gap-4 sm:grid-cols-2">
            <TextInput
              label="Name"
              value={name}
              maxLength={NAME_MAX}
              disabled={readOnly}
              placeholder="Daily standup reminder"
              onChange={(e) => setName(e.target.value)}
              dataQa="workflow-name"
              data-autofocus
            />
            <TextArea
              label="Description"
              optional
              rows={1}
              value={description}
              disabled={readOnly}
              onChange={(e) => setDescription(e.target.value)}
              dataQa="workflow-description"
            />
          </div>
          <TriggerCard
            trigger={definition.trigger}
            channels={postable}
            webhookUrl={workflow?.webhook_url ?? null}
            readOnly={readOnly}
            onRotate={workflow ? handleRotate : undefined}
            rotating={actions.rotate.isPending}
            onChange={(trigger) => setDefinition((d) => ({ ...d, trigger }))}
          />
          <ol className="space-y-3" aria-label="Steps">
            {definition.steps.map((step, index) => (
              <li key={index}>
                <StepCard
                  step={step}
                  index={index}
                  count={definition.steps.length}
                  definition={definition}
                  workspaceId={workspaceId}
                  instanceUrl={instanceUrl}
                  channels={postable}
                  people={people}
                  members={members}
                  variables={stepVariables(definition, index)}
                  formBefore={formsBefore(definition.steps, index).length > 0}
                  triggerPerson={triggerPerson(definition.trigger)}
                  readOnly={readOnly}
                  onChange={(next) => setStep(index, next)}
                  onMove={(to) => setDefinition((d) => ({ ...d, steps: moveStep(d.steps, index, to) }))}
                  onRemove={() => setDefinition((d) => ({ ...d, steps: removeStep(d.steps, index) }))}
                />
              </li>
            ))}
          </ol>
          {!readOnly && definition.steps.length < MAX_STEPS && (
            <Button
              variant="ghost"
              icon={<Plus className="h-4 w-4" />}
              onClick={() =>
                setDefinition((d) => ({
                  ...d,
                  steps: [...d.steps, blankStep('send_message', postable[0]?.id ?? '')],
                }))
              }
              data-qa="workflow-step-add"
            >
              Add a step
            </Button>
          )}
          {readOnly && (
            <p className="text-xs text-muted" data-qa="workflow-read-only">
              {mine
                ? 'Only admins can build workflows in this workspace.'
                : workflow?.creator_gone
                  ? 'The person who built this workflow left or was suspended. Take it over to change it or turn it back on.'
                  : 'Only the person who built this workflow can change it.'}
            </p>
          )}
          {error && (
            <p
              role="alert"
              className="rounded-lg bg-red-500/10 px-3 py-2 text-sm text-danger"
              data-qa="workflow-error"
            >
              {error}
            </p>
          )}
        </div>
      )}
      {confirmDelete && workflow && (
        <ConfirmDialog
          title={`Delete ${workflow.name}?`}
          description="It stops running and its history goes with it."
          confirmLabel="Delete workflow"
          onConfirm={async () => {
            await actions.remove.mutateAsync(workflow.id);
            onClose();
          }}
          onClose={() => setConfirmDelete(false)}
          dataQa="workflow-delete-confirm"
        />
      )}
    </Modal>
  );
}
