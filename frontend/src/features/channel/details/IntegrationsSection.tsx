import { useState, type FormEvent, type ReactNode } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { ArrowDownToLine, ArrowUpFromLine, RefreshCw, SquareSlash, Trash2, Workflow } from 'lucide-react';
import { Button } from '@/shared/components/Button/Button';
import { ConfirmDialog } from '@/shared/components/ConfirmDialog/ConfirmDialog';
import { CopyField } from '@/shared/components/CopyField/CopyField';
import { IconButton } from '@/shared/components/IconButton/IconButton';
import { TextInput } from '@/shared/components/Form/TextInput';
import { Spinner } from '@/shared/components/Spinner/Spinner';
import { QUERY_KEYS } from '@/shared/constants';
import { formatDateTime } from '@/lib/datetime';
import { displayNameOf } from '@/lib/userHelpers';
import { useWorkspaceStore, type Channel } from '@/stores/workspace';
import {
  useCreateHook,
  useDeleteHook,
  useRevealHook,
  useRotateHook,
  useWorkspaceMembers,
} from '@/features/workspace';
import { useChannelIntegrations, type ChannelIntegrations } from '../hooks/useChannelIntegrations';

type IncomingWebhook = ChannelIntegrations['incoming_webhooks'][number];

interface Props {
  channel: Channel;
  onOpenWorkflow?: (workflowId: string) => void;
}

export function IntegrationsSection({ channel, onOpenWorkflow }: Props) {
  const instanceUrl = useWorkspaceStore((s) => s.currentWorkspace?.instanceUrl);
  const integrations = useChannelIntegrations(channel.id);
  const { data: members = [] } = useWorkspaceMembers(channel.workspace_id, instanceUrl);
  const queryClient = useQueryClient();
  const createHook = useCreateHook(channel.workspace_id, instanceUrl);
  const deleteHook = useDeleteHook(channel.workspace_id, instanceUrl);
  const revealHook = useRevealHook(instanceUrl);
  const rotateHook = useRotateHook(instanceUrl);

  const [name, setName] = useState('');
  const [shown, setShown] = useState<{ hookId: string; url: string } | null>(null);
  const [error, setError] = useState<string | null>(null);

  const busy = createHook.isPending || deleteHook.isPending || revealHook.isPending || rotateHook.isPending;
  const nameOf = (userId: string) => displayNameOf(members.find((m) => m.user_id === userId)?.display_name);
  const refresh = () =>
    queryClient.invalidateQueries({ queryKey: QUERY_KEYS.channelIntegrations(channel.id) });
  const failWith = (fallback: string) => (err: unknown) =>
    setError((err as { message?: string })?.message || fallback);

  const show = (hookId: string, url: string | null | undefined) => setShown(url ? { hookId, url } : null);

  const handleAdd = async (e: FormEvent) => {
    e.preventDefault();
    if (!name.trim()) return;
    setError(null);
    try {
      const hook = await createHook.mutateAsync({
        hook_type: 'incoming_webhook',
        name: name.trim(),
        config: { channel_id: channel.id },
      });
      await refresh();
      show(hook.id, (await revealHook.mutateAsync(hook.id)).incoming_url);
      setName('');
    } catch (err: unknown) {
      failWith('Failed to add the webhook')(err);
    }
  };

  const handleRotate = async (hookId: string) => {
    setError(null);
    try {
      show(hookId, (await rotateHook.mutateAsync(hookId)).incoming_url);
    } catch (err: unknown) {
      failWith('Failed to rotate the webhook')(err);
    }
  };

  const handleRemove = async (hookId: string) => {
    setError(null);
    await deleteHook.mutateAsync(hookId);
    if (shown?.hookId === hookId) setShown(null);
    await refresh();
  };

  if (integrations.isLoading || !integrations.data) {
    return (
      <div className="flex justify-center py-8 text-accent">
        <Spinner />
      </div>
    );
  }

  const {
    incoming_webhooks: incoming,
    outgoing_webhooks: outgoing,
    slash_commands: commands,
    workflows,
    can_add_webhook: canManage,
  } = integrations.data;

  return (
    <div className="space-y-6" data-qa="channel-integrations">
      {error && (
        <p role="alert" className="text-sm text-danger">
          {error}
        </p>
      )}

      <Group
        icon={<ArrowDownToLine className="h-3.5 w-3.5" />}
        title="Incoming webhooks"
        hint="Post into this channel from anything that can send a little JSON."
        empty={canManage ? null : 'None post into this channel.'}
        count={incoming.length}
        qa="channel-integrations-incoming"
      >
        {incoming.map((hook) => (
          <IncomingRow
            key={hook.id}
            hook={hook}
            author={nameOf(hook.created_by)}
            url={shown?.hookId === hook.id ? shown.url : null}
            canManage={canManage}
            busy={busy}
            onRotate={() => void handleRotate(hook.id)}
            onRemove={() => handleRemove(hook.id)}
          />
        ))}
        {canManage && (
          <form onSubmit={handleAdd} noValidate className="flex items-end gap-2 pt-1">
            <TextInput
              label="Webhook name"
              labelHidden
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="Name, e.g. CI alerts"
              dataQa="channel-webhook-name"
              className="flex-1"
            />
            <Button
              type="submit"
              variant="primary"
              loading={createHook.isPending || revealHook.isPending}
              disabled={busy || !name.trim()}
              data-qa="channel-webhook-add"
            >
              Add an incoming webhook
            </Button>
          </form>
        )}
      </Group>

      <Group
        icon={<ArrowUpFromLine className="h-3.5 w-3.5" />}
        title="Outgoing webhooks"
        hint="Every message here is also sent to a service outside this workspace."
        empty="Nothing here is sent outside."
        count={outgoing.length}
        qa="channel-integrations-outgoing"
      >
        {outgoing.map((hook) => (
          <Row key={hook.id} title={hook.name} detail={hook.description} qa="channel-integration-outgoing" />
        ))}
      </Group>

      <Group
        icon={<SquareSlash className="h-3.5 w-3.5" />}
        title="Slash commands"
        hint="What you type after the command is sent to a service outside this workspace."
        empty="No commands are set up for this channel."
        count={commands.length}
        qa="channel-integrations-commands"
      >
        {commands.map((command) => (
          <Row
            key={command.id}
            title={`/${command.command}`}
            detail={command.name}
            qa="channel-integration-command"
          />
        ))}
      </Group>

      <Group
        icon={<Workflow className="h-3.5 w-3.5" />}
        title="Workflows"
        hint="Workflows that start from this channel or post into it."
        empty="No workflow uses this channel."
        count={workflows.length}
        qa="channel-integrations-workflows"
      >
        {workflows.map((workflow) => (
          <Row
            key={workflow.id}
            title={workflow.name}
            detail={`${workflow.enabled ? 'On' : 'Off'} · built by ${nameOf(workflow.created_by)}`}
            qa="channel-integration-workflow"
            action={
              onOpenWorkflow && (
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => onOpenWorkflow(workflow.id)}
                  data-qa="channel-integration-workflow-open"
                >
                  Open
                </Button>
              )
            }
          />
        ))}
      </Group>
    </div>
  );
}

function Group({
  icon,
  title,
  hint,
  empty,
  count,
  qa,
  children,
}: {
  icon: ReactNode;
  title: string;
  hint: string;
  empty: string | null;
  count: number;
  qa: string;
  children: ReactNode;
}) {
  return (
    <section data-qa={qa}>
      <h4 className="flex items-center gap-2 text-xs font-semibold uppercase tracking-wider text-muted">
        {icon}
        {title}
      </h4>
      <p className="mt-1 text-xs text-muted">{hint}</p>
      <div className="mt-2 space-y-2">
        {count === 0 && empty && <p className="text-sm text-fg-dim">{empty}</p>}
        {children}
      </div>
    </section>
  );
}

function Row({
  title,
  detail,
  qa,
  action,
}: {
  title: string;
  detail?: string | null;
  qa: string;
  action?: ReactNode;
}) {
  return (
    <div className="flex items-center gap-2 rounded-lg border border-line/60 px-3 py-2" data-qa={qa}>
      <div className="min-w-0 flex-1">
        <div className="truncate text-sm font-medium text-fg-soft">{title}</div>
        {detail && <div className="truncate text-xs text-muted">{detail}</div>}
      </div>
      {action}
    </div>
  );
}

function IncomingRow({
  hook,
  author,
  url,
  canManage,
  busy,
  onRotate,
  onRemove,
}: {
  hook: IncomingWebhook;
  author: string;
  url: string | null;
  canManage: boolean;
  busy: boolean;
  onRotate: () => void;
  onRemove: () => Promise<void>;
}) {
  const [confirmRemove, setConfirmRemove] = useState(false);
  return (
    <div className="rounded-lg border border-line/60 px-3 py-2" data-qa="channel-integration-incoming">
      <div className="flex items-start gap-2">
        <div className="min-w-0 flex-1">
          <div className="truncate text-sm font-medium text-fg-soft">{hook.name}</div>
          <div className="truncate text-xs text-muted">
            Added by {author} · {formatDateTime(hook.created_at)}
          </div>
          {hook.description && <div className="text-xs text-fg-dim">{hook.description}</div>}
        </div>
        {canManage && (
          <>
            <IconButton
              label={`Make a new URL for ${hook.name}`}
              icon={<RefreshCw className="h-4 w-4" />}
              onClick={onRotate}
              disabled={busy}
              data-qa="channel-webhook-rotate"
            />
            <IconButton
              label={`Remove ${hook.name}`}
              icon={<Trash2 className="h-4 w-4" />}
              tone="danger"
              onClick={() => setConfirmRemove(true)}
              disabled={busy}
              data-qa="channel-webhook-remove"
            />
          </>
        )}
      </div>
      {url && (
        <div data-qa="channel-webhook-url">
          <CopyField label="Webhook URL" value={url} valueQa="channel-webhook-url-value" />
          <p className="mt-1 text-xs text-muted">
            Copy it now: it is not shown again. Anyone with it can post here.
          </p>
        </div>
      )}
      {confirmRemove && (
        <ConfirmDialog
          title={`Remove ${hook.name}?`}
          description="Anything still posting to its URL will be refused."
          confirmLabel="Remove webhook"
          onConfirm={onRemove}
          onClose={() => setConfirmRemove(false)}
          dataQa="channel-webhook-remove-dialog"
        />
      )}
    </div>
  );
}
