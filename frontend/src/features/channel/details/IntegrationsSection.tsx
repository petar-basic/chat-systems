import { useState, type FormEvent, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
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
  type AppRef,
} from '@/features/workspace';
import { useChannelIntegrations, type ChannelIntegrations } from '../hooks/useChannelIntegrations';

type IncomingWebhook = ChannelIntegrations['incoming_webhooks'][number];

interface Props {
  channel: Channel;
  onOpenWorkflow?: (workflowId: string) => void;
  onOpenApp?: (app: AppRef) => void;
}

export function IntegrationsSection({ channel, onOpenWorkflow, onOpenApp }: Props) {
  const { t } = useTranslation('channel');
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

  const details = (app: AppRef, name: string) =>
    onOpenApp && (
      <Button
        variant="ghost"
        size="sm"
        onClick={() => onOpenApp(app)}
        aria-label={t('details.integrations.detailsOf', { name })}
        data-qa="channel-integration-details"
      >
        {t('details.integrations.details')}
      </Button>
    );

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
      failWith(t('details.integrations.incoming.addFailed'))(err);
    }
  };

  const handleRotate = async (hookId: string) => {
    setError(null);
    try {
      show(hookId, (await rotateHook.mutateAsync(hookId)).incoming_url);
    } catch (err: unknown) {
      failWith(t('details.integrations.incoming.rotateFailed'))(err);
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
        title={t('details.integrations.incoming.title')}
        hint={t('details.integrations.incoming.hint')}
        empty={canManage ? null : t('details.integrations.incoming.empty')}
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
            details={details({ kind: 'incoming_webhook', id: hook.id }, hook.name)}
            onRotate={() => void handleRotate(hook.id)}
            onRemove={() => handleRemove(hook.id)}
          />
        ))}
        {canManage && (
          <form onSubmit={handleAdd} noValidate className="flex items-end gap-2 pt-1">
            <TextInput
              label={t('details.integrations.incoming.nameLabel')}
              labelHidden
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder={t('details.integrations.incoming.namePlaceholder')}
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
              {t('details.integrations.incoming.add')}
            </Button>
          </form>
        )}
      </Group>

      <Group
        icon={<ArrowUpFromLine className="h-3.5 w-3.5" />}
        title={t('details.integrations.outgoing.title')}
        hint={t('details.integrations.outgoing.hint')}
        empty={t('details.integrations.outgoing.empty')}
        count={outgoing.length}
        qa="channel-integrations-outgoing"
      >
        {outgoing.map((hook) => (
          <Row
            key={hook.id}
            title={hook.name}
            detail={hook.description}
            qa="channel-integration-outgoing"
            action={details({ kind: 'outgoing_webhook', id: hook.id }, hook.name)}
          />
        ))}
      </Group>

      <Group
        icon={<SquareSlash className="h-3.5 w-3.5" />}
        title={t('details.integrations.commands.title')}
        hint={t('details.integrations.commands.hint')}
        empty={t('details.integrations.commands.empty')}
        count={commands.length}
        qa="channel-integrations-commands"
      >
        {commands.map((command) => (
          <Row
            key={command.id}
            title={`/${command.command}`}
            detail={command.name}
            qa="channel-integration-command"
            action={details({ kind: 'slash_command', id: command.id }, `/${command.command}`)}
          />
        ))}
      </Group>

      <Group
        icon={<Workflow className="h-3.5 w-3.5" />}
        title={t('details.integrations.workflows.title')}
        hint={t('details.integrations.workflows.hint')}
        empty={t('details.integrations.workflows.empty')}
        count={workflows.length}
        qa="channel-integrations-workflows"
      >
        {workflows.map((workflow) => (
          <Row
            key={workflow.id}
            title={workflow.name}
            detail={t(
              workflow.enabled ? 'details.integrations.workflows.on' : 'details.integrations.workflows.off',
              {
                name: nameOf(workflow.created_by),
              },
            )}
            qa="channel-integration-workflow"
            action={
              <>
                {details({ kind: 'workflow', id: workflow.id }, workflow.name)}
                {onOpenWorkflow && (
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => onOpenWorkflow(workflow.id)}
                    data-qa="channel-integration-workflow-open"
                  >
                    {t('details.integrations.open')}
                  </Button>
                )}
              </>
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
  details,
  onRotate,
  onRemove,
}: {
  hook: IncomingWebhook;
  author: string;
  url: string | null;
  canManage: boolean;
  busy: boolean;
  details: ReactNode;
  onRotate: () => void;
  onRemove: () => Promise<void>;
}) {
  const { t } = useTranslation('channel');
  const [confirmRemove, setConfirmRemove] = useState(false);
  return (
    <div className="rounded-lg border border-line/60 px-3 py-2" data-qa="channel-integration-incoming">
      <div className="flex items-start gap-2">
        <div className="min-w-0 flex-1">
          <div className="truncate text-sm font-medium text-fg-soft">{hook.name}</div>
          <div className="truncate text-xs text-muted">
            {t('details.integrations.incoming.addedBy', {
              name: author,
              time: formatDateTime(hook.created_at),
            })}
          </div>
          {hook.description && <div className="text-xs text-fg-dim">{hook.description}</div>}
        </div>
        {details}
        {canManage && (
          <>
            <IconButton
              label={t('details.integrations.incoming.rotate', { name: hook.name })}
              icon={<RefreshCw className="h-4 w-4" />}
              onClick={onRotate}
              disabled={busy}
              data-qa="channel-webhook-rotate"
            />
            <IconButton
              label={t('details.integrations.incoming.remove', { name: hook.name })}
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
          <CopyField
            label={t('details.integrations.incoming.url')}
            value={url}
            valueQa="channel-webhook-url-value"
          />
          <p className="mt-1 text-xs text-muted">{t('details.integrations.incoming.urlHint')}</p>
        </div>
      )}
      {confirmRemove && (
        <ConfirmDialog
          title={t('details.integrations.incoming.removeTitle', { name: hook.name })}
          description={t('details.integrations.incoming.removeDescription')}
          confirmLabel={t('details.integrations.incoming.removeConfirm')}
          onConfirm={onRemove}
          onClose={() => setConfirmRemove(false)}
          dataQa="channel-webhook-remove-dialog"
        />
      )}
    </div>
  );
}
