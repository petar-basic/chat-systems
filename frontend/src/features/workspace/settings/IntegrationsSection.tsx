import { Fragment, useState, type FormEvent } from 'react';
import { Link } from 'react-router';
import { useTranslation } from 'react-i18next';
import { Trans } from '@/shared/components/Trans/Trans';
import i18n from '@/lib/i18n';
import { Spinner } from '@/shared/components/Spinner/Spinner';
import { Button } from '@/shared/components/Button/Button';
import { CopyField } from '@/shared/components/CopyField/CopyField';
import { ConfirmDialog } from '@/shared/components/ConfirmDialog/ConfirmDialog';
import { TextInput } from '@/shared/components/Form/TextInput';
import { Select } from '@/shared/components/Form/Select';
import { Checkbox } from '@/shared/components/Form/Checkbox';
import { Trash2, Eye, RefreshCw, ArrowDownToLine, ArrowUpFromLine } from 'lucide-react';
import type { Channel } from '@/stores/workspace';
import {
  useHooks,
  useCreateHook,
  useDeleteHook,
  useEnableHook,
  useRevealHook,
  useRotateHook,
  type Hook,
  type HookSecrets,
} from '@/features/workspace/hooks/useHooks';

interface Props {
  workspaceId: string;
  instanceUrl?: string;
  channels: Channel[];
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function configOf(source: { config: unknown }): Record<string, unknown> {
  return isRecord(source.config) ? source.config : {};
}

function ChannelLink({
  workspaceId,
  channel,
}: {
  workspaceId: string;
  channel: { id: string; name: string };
}) {
  return (
    <Link to={`/app/${workspaceId}/${channel.id}`} className="hover:underline" data-qa="hook-channel-link">
      #{channel.name}
    </Link>
  );
}

function HookRow({
  hook,
  workspaceId,
  channels,
  scope,
  secrets,
  busy,
  onReveal,
  onRotate,
  onEnable,
  onDelete,
}: {
  hook: Hook;
  workspaceId: string;
  channels: { id: string; name: string }[];
  scope?: boolean;
  secrets: HookSecrets | null;
  busy: boolean;
  onReveal: () => void;
  onRotate: () => void;
  onEnable: () => void;
  onDelete: () => Promise<void>;
}) {
  const { t } = useTranslation('workspace');
  const [confirmDelete, setConfirmDelete] = useState(false);
  const isIncoming = hook.hook_type === 'incoming_webhook';
  const linked = channels.map((channel, i) => (
    <Fragment key={channel.id}>
      {i > 0 && i18n.t('common:listSeparator')}
      <ChannelLink workspaceId={workspaceId} channel={channel} />
    </Fragment>
  ));
  const signingSecret = secrets ? configOf(secrets).secret : undefined;

  return (
    <div className="px-4 py-3 border-b border-line/40" data-qa="hook-row" data-hook-id={hook.id}>
      <div className="flex items-start gap-2">
        <div className="flex-1 min-w-0">
          <div className="text-sm font-medium text-fg-soft truncate">{hook.name}</div>
          <div className="text-xs text-muted truncate" data-qa="hook-target">
            {isIncoming
              ? linked.length > 0
                ? linked
                : t('settings.integrations.row.noChannel')
              : String(configOf(hook).url ?? '')}
          </div>
          {scope && linked.length > 0 && (
            <div className="text-xs text-warning/80 truncate" data-qa="hook-scope">
              {t('settings.integrations.row.forwards')} {linked}
            </div>
          )}
          {!hook.is_active && (
            <div className="flex items-center gap-2 mt-1">
              <span className="text-xs text-danger" data-qa="hook-inactive">
                {hook.disabled_reason
                  ? t('settings.integrations.row.disabledReason', {
                      reason: `${hook.disabled_reason.charAt(0).toLowerCase()}${hook.disabled_reason.slice(1)}`,
                    })
                  : t('settings.integrations.row.disabled')}
              </span>
              <Button size="sm" onClick={onEnable} disabled={busy} data-qa="hook-enable">
                {t('settings.integrations.row.turnOn')}
              </Button>
            </div>
          )}
        </div>
        <button
          onClick={onReveal}
          disabled={busy}
          aria-label={t('settings.integrations.row.revealLabel', { name: hook.name })}
          data-tooltip={t('settings.integrations.row.reveal')}
          data-qa="hook-reveal"
          className="p-1.5 rounded text-muted hover:text-fg hover:bg-raised transition cursor-pointer disabled:opacity-50"
        >
          <Eye className="w-4 h-4" />
        </button>
        <button
          onClick={onRotate}
          disabled={busy}
          aria-label={t('settings.integrations.row.rotateLabel', { name: hook.name })}
          data-tooltip={t('settings.integrations.row.rotate')}
          data-qa="hook-rotate"
          className="p-1.5 rounded text-muted hover:text-warning hover:bg-raised transition cursor-pointer disabled:opacity-50"
        >
          <RefreshCw className="w-4 h-4" />
        </button>
        <button
          onClick={() => setConfirmDelete(true)}
          disabled={busy}
          aria-label={t('settings.integrations.row.deleteLabel', { name: hook.name })}
          data-tooltip={t('settings.integrations.row.delete')}
          data-qa="hook-delete"
          className="p-1.5 rounded text-muted hover:text-danger hover:bg-raised transition cursor-pointer disabled:opacity-50"
        >
          <Trash2 className="w-4 h-4" />
        </button>
      </div>

      {confirmDelete && (
        <ConfirmDialog
          title={t('settings.integrations.row.deleteTitle', { name: hook.name })}
          description={t(
            isIncoming
              ? 'settings.integrations.row.deleteIncoming'
              : 'settings.integrations.row.deleteOutgoing',
          )}
          confirmLabel={t('settings.integrations.row.deleteConfirm')}
          onConfirm={onDelete}
          onClose={() => setConfirmDelete(false)}
          dataQa="hook-delete-dialog"
        />
      )}

      {secrets && (
        <div data-qa="hook-secrets">
          {secrets.incoming_url && (
            <CopyField
              valueQa="hook-secret-value"
              copyQa="hook-secret-copy"
              label={t('settings.integrations.row.webhookUrl')}
              value={secrets.incoming_url}
            />
          )}
          {typeof signingSecret === 'string' && (
            <CopyField
              valueQa="hook-secret-value"
              copyQa="hook-secret-copy"
              label={t('settings.integrations.row.signingSecret')}
              value={signingSecret}
            />
          )}
        </div>
      )}
    </div>
  );
}

export function IntegrationsSection({ workspaceId, instanceUrl, channels }: Props) {
  const { t } = useTranslation('workspace');
  const { data: hooks = [], isLoading } = useHooks(workspaceId, instanceUrl);
  const createHook = useCreateHook(workspaceId, instanceUrl);
  const deleteHook = useDeleteHook(workspaceId, instanceUrl);
  const revealHook = useRevealHook(instanceUrl);
  const rotateHook = useRotateHook(instanceUrl);
  const enableHook = useEnableHook(workspaceId, instanceUrl);

  const [secretsByHook, setSecretsByHook] = useState<Record<string, HookSecrets>>({});
  const [error, setError] = useState<string | null>(null);
  const [incomingName, setIncomingName] = useState('');
  const [incomingChannel, setIncomingChannel] = useState('');
  const [outgoingName, setOutgoingName] = useState('');
  const [outgoingUrl, setOutgoingUrl] = useState('');
  const [outgoingChannels, setOutgoingChannels] = useState<string[]>([]);

  const postable = channels.filter((c) => c.channel_type === 'public' || c.channel_type === 'private');
  const incoming = hooks.filter((h) => h.hook_type === 'incoming_webhook');
  const outgoing = hooks.filter((h) => h.hook_type === 'outgoing_webhook');
  const busy =
    createHook.isPending ||
    deleteHook.isPending ||
    revealHook.isPending ||
    rotateHook.isPending ||
    enableHook.isPending;

  const failWith = (fallback: string) => (err: unknown) =>
    setError((err as { message?: string })?.message || fallback);

  const remember = (secrets: HookSecrets) =>
    setSecretsByHook((prev) => ({ ...prev, [secrets.hook_id]: secrets }));

  const handleCreateIncoming = async (e: FormEvent) => {
    e.preventDefault();
    if (!incomingName.trim() || !incomingChannel) return;
    setError(null);
    try {
      const hook = await createHook.mutateAsync({
        hook_type: 'incoming_webhook',
        name: incomingName.trim(),
        config: { channel_id: incomingChannel },
      });
      remember(await revealHook.mutateAsync(hook.id));
      setIncomingName('');
      setIncomingChannel('');
    } catch (err: unknown) {
      failWith(t('settings.integrations.failed.create'))(err);
    }
  };

  const handleCreateOutgoing = async (e: FormEvent) => {
    e.preventDefault();
    if (!outgoingName.trim() || !outgoingUrl.trim() || outgoingChannels.length === 0) return;
    setError(null);
    try {
      const hook = await createHook.mutateAsync({
        hook_type: 'outgoing_webhook',
        name: outgoingName.trim(),
        config: { url: outgoingUrl.trim(), channel_ids: outgoingChannels },
      });
      remember(await revealHook.mutateAsync(hook.id));
      setOutgoingName('');
      setOutgoingUrl('');
      setOutgoingChannels([]);
    } catch (err: unknown) {
      failWith(t('settings.integrations.failed.create'))(err);
    }
  };

  const toggleOutgoingChannel = (channelId: string) =>
    setOutgoingChannels((prev) =>
      prev.includes(channelId) ? prev.filter((id) => id !== channelId) : [...prev, channelId],
    );

  const channelsOf = (hook: Hook) => {
    const config = configOf(hook);
    const ids = Array.isArray(config.channel_ids) ? config.channel_ids : [config.channel_id];
    return ids
      .filter((id): id is string => typeof id === 'string')
      .map((id) => ({
        id,
        name: channels.find((c) => c.id === id)?.name ?? t('settings.integrations.row.unknownChannel'),
      }));
  };

  const handleReveal = async (hookId: string) => {
    setError(null);
    try {
      remember(await revealHook.mutateAsync(hookId));
    } catch (err: unknown) {
      failWith(t('settings.integrations.failed.reveal'))(err);
    }
  };

  const handleRotate = async (hookId: string) => {
    setError(null);
    try {
      remember(await rotateHook.mutateAsync(hookId));
    } catch (err: unknown) {
      failWith(t('settings.integrations.failed.rotate'))(err);
    }
  };

  const handleEnable = async (hookId: string) => {
    setError(null);
    try {
      const { secrets } = await enableHook.mutateAsync(hookId);
      if (secrets) remember(secrets);
    } catch (err: unknown) {
      failWith(t('settings.integrations.failed.turnOn'))(err);
    }
  };

  const handleDelete = async (hookId: string) => {
    setError(null);
    await deleteHook.mutateAsync(hookId);
    setSecretsByHook((prev) => {
      const next = { ...prev };
      delete next[hookId];
      return next;
    });
  };

  return (
    <div className="-mx-4 flex flex-col" data-qa="integrations-panel">
      <div className="flex-1 overflow-y-auto">
        {error && <div className="px-4 py-2 text-xs text-danger">{error}</div>}

        <div className="px-4 py-3 flex items-center gap-2 text-xs font-semibold text-muted uppercase tracking-wider">
          <ArrowDownToLine className="w-3.5 h-3.5" />
          {t('settings.integrations.incoming.title')}
        </div>
        <p className="px-4 -mt-1 pb-2 text-xs text-muted">
          <Trans
            t={t}
            i18nKey="settings.integrations.incoming.intro"
            components={{ code: <code className="text-fg-dim" /> }}
          />
        </p>

        {isLoading ? (
          <div className="flex justify-center py-6">
            <Spinner className="text-accent" />
          </div>
        ) : (
          incoming.map((hook) => (
            <HookRow
              key={hook.id}
              hook={hook}
              workspaceId={workspaceId}
              channels={channelsOf(hook)}
              secrets={secretsByHook[hook.id] ?? null}
              busy={busy}
              onReveal={() => void handleReveal(hook.id)}
              onRotate={() => void handleRotate(hook.id)}
              onEnable={() => void handleEnable(hook.id)}
              onDelete={() => handleDelete(hook.id)}
            />
          ))
        )}

        <form onSubmit={handleCreateIncoming} className="px-4 py-3 space-y-2 border-b border-line/50">
          <TextInput
            label={t('settings.integrations.incoming.nameLabel')}
            labelHidden
            value={incomingName}
            onChange={(e) => setIncomingName(e.target.value)}
            placeholder={t('settings.integrations.incoming.namePlaceholder')}
            dataQa="incoming-hook-name"
          />
          <Select
            label={t('settings.integrations.incoming.channelLabel')}
            labelHidden
            value={incomingChannel}
            onChange={(e) => setIncomingChannel(e.target.value)}
            dataQa="incoming-hook-channel"
          >
            <option value="">{t('settings.integrations.incoming.channelPlaceholder')}</option>
            {postable.map((c) => (
              <option key={c.id} value={c.id}>
                #{c.name}
              </option>
            ))}
          </Select>
          <Button
            variant="primary"
            fullWidth
            type="submit"
            disabled={busy || !incomingName.trim() || !incomingChannel}
            data-qa="incoming-hook-create"
          >
            {t('settings.integrations.incoming.create')}
          </Button>
        </form>

        <div className="px-4 py-3 flex items-center gap-2 text-xs font-semibold text-muted uppercase tracking-wider">
          <ArrowUpFromLine className="w-3.5 h-3.5" />
          {t('settings.integrations.outgoing.title')}
        </div>
        <p className="px-4 -mt-1 pb-2 text-xs text-muted">{t('settings.integrations.outgoing.intro')}</p>

        {outgoing.map((hook) => (
          <HookRow
            key={hook.id}
            hook={hook}
            workspaceId={workspaceId}
            channels={channelsOf(hook)}
            scope
            secrets={secretsByHook[hook.id] ?? null}
            busy={busy}
            onReveal={() => void handleReveal(hook.id)}
            onRotate={() => void handleRotate(hook.id)}
            onEnable={() => void handleEnable(hook.id)}
            onDelete={() => handleDelete(hook.id)}
          />
        ))}

        <form onSubmit={handleCreateOutgoing} className="px-4 py-3 space-y-2">
          <TextInput
            label={t('settings.integrations.outgoing.nameLabel')}
            labelHidden
            value={outgoingName}
            onChange={(e) => setOutgoingName(e.target.value)}
            placeholder={t('settings.integrations.outgoing.namePlaceholder')}
            dataQa="outgoing-hook-name"
          />
          <TextInput
            label={t('settings.integrations.outgoing.urlLabel')}
            labelHidden
            type="url"
            value={outgoingUrl}
            onChange={(e) => setOutgoingUrl(e.target.value)}
            placeholder="https://example.com/hooks/chat"
            dataQa="outgoing-hook-url"
          />
          <div
            className="max-h-40 overflow-y-auto rounded-lg border border-line-strong bg-raised/50 divide-y divide-line-strong/50"
            data-qa="outgoing-hook-channels"
          >
            {postable.length === 0 && (
              <div className="px-3 py-2 text-xs text-muted">
                {t('settings.integrations.outgoing.noChannels')}
              </div>
            )}
            {postable.map((c) => (
              <Checkbox
                key={c.id}
                label={`#${c.name}`}
                checked={outgoingChannels.includes(c.id)}
                onChange={() => toggleOutgoingChannel(c.id)}
                dataQa={`outgoing-hook-channel-${c.id}`}
                className="px-3 py-2 hover:bg-raised"
              />
            ))}
          </div>
          <Button
            variant="primary"
            fullWidth
            type="submit"
            disabled={busy || !outgoingName.trim() || !outgoingUrl.trim() || outgoingChannels.length === 0}
            data-qa="outgoing-hook-create"
          >
            {t('settings.integrations.outgoing.create')}
          </Button>
        </form>
      </div>
    </div>
  );
}
