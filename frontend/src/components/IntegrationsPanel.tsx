import { useState, type FormEvent } from 'react';
import { Spinner } from '@/shared/components/Spinner/Spinner';
import { Button } from '@/shared/components/Button/Button';
import { PanelHeader } from '@/shared/components/PanelHeader/PanelHeader';
import { CopyField } from '@/shared/components/CopyField/CopyField';
import { ConfirmDialog } from '@/shared/components/ConfirmDialog/ConfirmDialog';
import { TextInput } from '@/shared/components/Form/TextInput';
import { Select } from '@/shared/components/Form/Select';
import { Checkbox } from '@/shared/components/Form/Checkbox';
import { Plug, Trash2, Eye, RefreshCw, ArrowDownToLine, ArrowUpFromLine } from 'lucide-react';
import type { Channel } from '@/stores/workspace';
import {
  useHooks,
  useCreateHook,
  useDeleteHook,
  useRevealHook,
  useRotateHook,
  type Hook,
  type HookSecrets,
} from '@/hooks/queries/useHooks';

interface Props {
  workspaceId: string;
  instanceUrl?: string;
  channels: Channel[];
  onClose: () => void;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function configOf(source: { config: unknown }): Record<string, unknown> {
  return isRecord(source.config) ? source.config : {};
}

function HookRow({
  hook,
  channelName,
  scope,
  secrets,
  busy,
  onReveal,
  onRotate,
  onDelete,
}: {
  hook: Hook;
  channelName: string | null;
  scope?: string;
  secrets: HookSecrets | null;
  busy: boolean;
  onReveal: () => void;
  onRotate: () => void;
  onDelete: () => Promise<void>;
}) {
  const [confirmDelete, setConfirmDelete] = useState(false);
  const isIncoming = hook.hook_type === 'incoming_webhook';
  const target = isIncoming ? `#${channelName ?? 'unknown channel'}` : String(configOf(hook).url ?? '');
  const signingSecret = secrets ? configOf(secrets).secret : undefined;

  return (
    <div className="px-4 py-3 border-b border-line/40" data-qa="hook-row" data-hook-id={hook.id}>
      <div className="flex items-start gap-2">
        <div className="flex-1 min-w-0">
          <div className="text-sm font-medium text-fg-soft truncate">{hook.name}</div>
          <div className="text-xs text-muted truncate" data-qa="hook-target">
            {target}
          </div>
          {scope && (
            <div className="text-xs text-warning/80 truncate" data-qa="hook-scope">
              Forwards {scope}
            </div>
          )}
          {!hook.is_active && (
            <div className="text-xs text-danger" data-qa="hook-inactive">
              Disabled — re-create it with the channels it may read
            </div>
          )}
        </div>
        <button
          onClick={onReveal}
          disabled={busy}
          aria-label={`Reveal credentials for ${hook.name}`}
          title="Reveal"
          data-qa="hook-reveal"
          className="p-1.5 rounded text-muted hover:text-fg hover:bg-raised transition cursor-pointer disabled:opacity-50"
        >
          <Eye className="w-4 h-4" />
        </button>
        <button
          onClick={onRotate}
          disabled={busy}
          aria-label={`Rotate credentials for ${hook.name}`}
          title="Rotate — the old value stops working immediately"
          data-qa="hook-rotate"
          className="p-1.5 rounded text-muted hover:text-warning hover:bg-raised transition cursor-pointer disabled:opacity-50"
        >
          <RefreshCw className="w-4 h-4" />
        </button>
        <button
          onClick={() => setConfirmDelete(true)}
          disabled={busy}
          aria-label={`Delete ${hook.name}`}
          title="Delete"
          data-qa="hook-delete"
          className="p-1.5 rounded text-muted hover:text-danger hover:bg-raised transition cursor-pointer disabled:opacity-50"
        >
          <Trash2 className="w-4 h-4" />
        </button>
      </div>

      {confirmDelete && (
        <ConfirmDialog
          title={`Delete ${hook.name}?`}
          description={
            isIncoming
              ? 'Anything still posting to its URL will be refused.'
              : 'Messages stop being sent to its URL.'
          }
          confirmLabel="Delete integration"
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
              label="Webhook URL"
              value={secrets.incoming_url}
            />
          )}
          {typeof signingSecret === 'string' && (
            <CopyField
              valueQa="hook-secret-value"
              copyQa="hook-secret-copy"
              label="Signing secret"
              value={signingSecret}
            />
          )}
        </div>
      )}
    </div>
  );
}

export default function IntegrationsPanel({ workspaceId, instanceUrl, channels, onClose }: Props) {
  const { data: hooks = [], isLoading } = useHooks(workspaceId, instanceUrl);
  const createHook = useCreateHook(workspaceId, instanceUrl);
  const deleteHook = useDeleteHook(workspaceId, instanceUrl);
  const revealHook = useRevealHook(instanceUrl);
  const rotateHook = useRotateHook(instanceUrl);

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
  const busy = createHook.isPending || deleteHook.isPending || revealHook.isPending || rotateHook.isPending;

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
      failWith('Failed to create the webhook')(err);
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
      failWith('Failed to create the webhook')(err);
    }
  };

  const toggleOutgoingChannel = (channelId: string) =>
    setOutgoingChannels((prev) =>
      prev.includes(channelId) ? prev.filter((id) => id !== channelId) : [...prev, channelId],
    );

  const scopeLabel = (hook: Hook) => {
    const channelIds = configOf(hook).channel_ids;
    const ids = Array.isArray(channelIds)
      ? channelIds.filter((id): id is string => typeof id === 'string')
      : [];
    if (ids.length === 0) return undefined;
    return ids.map((id) => `#${channels.find((c) => c.id === id)?.name ?? 'unknown'}`).join(', ');
  };

  const handleReveal = async (hookId: string) => {
    setError(null);
    try {
      remember(await revealHook.mutateAsync(hookId));
    } catch (err: unknown) {
      failWith('Failed to reveal the credentials')(err);
    }
  };

  const handleRotate = async (hookId: string) => {
    setError(null);
    try {
      remember(await rotateHook.mutateAsync(hookId));
    } catch (err: unknown) {
      failWith('Failed to rotate the credentials')(err);
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
    <div
      className="w-full lg:w-96 max-lg:fixed max-lg:inset-0 max-lg:z-40 bg-surface border-l border-line/50 flex flex-col"
      data-qa="integrations-panel"
    >
      <PanelHeader title="Integrations" icon={<Plug className="h-4 w-4 text-muted" />} onClose={onClose} />

      <div className="flex-1 overflow-y-auto">
        {error && <div className="px-4 py-2 text-xs text-danger">{error}</div>}

        <div className="px-4 py-3 flex items-center gap-2 text-xs font-semibold text-muted uppercase tracking-wider">
          <ArrowDownToLine className="w-3.5 h-3.5" />
          Incoming webhooks
        </div>
        <p className="px-4 -mt-1 pb-2 text-xs text-muted">
          Post into a channel from anything that can send{' '}
          <code className="text-fg-dim">{'{"text":"…"}'}</code>.
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
              channelName={channels.find((c) => c.id === configOf(hook).channel_id)?.name ?? null}
              secrets={secretsByHook[hook.id] ?? null}
              busy={busy}
              onReveal={() => void handleReveal(hook.id)}
              onRotate={() => void handleRotate(hook.id)}
              onDelete={() => handleDelete(hook.id)}
            />
          ))
        )}

        <form onSubmit={handleCreateIncoming} className="px-4 py-3 space-y-2 border-b border-line/50">
          <TextInput
            label="Incoming webhook name"
            labelHidden
            value={incomingName}
            onChange={(e) => setIncomingName(e.target.value)}
            placeholder="Name, e.g. CI alerts"
            dataQa="incoming-hook-name"
          />
          <Select
            label="Incoming webhook channel"
            labelHidden
            value={incomingChannel}
            onChange={(e) => setIncomingChannel(e.target.value)}
            dataQa="incoming-hook-channel"
          >
            <option value="">Post to channel…</option>
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
            Create incoming webhook
          </Button>
        </form>

        <div className="px-4 py-3 flex items-center gap-2 text-xs font-semibold text-muted uppercase tracking-wider">
          <ArrowUpFromLine className="w-3.5 h-3.5" />
          Outgoing webhooks
        </div>
        <p className="px-4 -mt-1 pb-2 text-xs text-muted">
          Messages in the channels you pick are POSTed to your URL, signed with the secret below. Everyone in
          those channels can see that the integration is attached.
        </p>

        {outgoing.map((hook) => (
          <HookRow
            key={hook.id}
            hook={hook}
            channelName={null}
            scope={scopeLabel(hook)}
            secrets={secretsByHook[hook.id] ?? null}
            busy={busy}
            onReveal={() => void handleReveal(hook.id)}
            onRotate={() => void handleRotate(hook.id)}
            onDelete={() => handleDelete(hook.id)}
          />
        ))}

        <form onSubmit={handleCreateOutgoing} className="px-4 py-3 space-y-2">
          <TextInput
            label="Outgoing webhook name"
            labelHidden
            value={outgoingName}
            onChange={(e) => setOutgoingName(e.target.value)}
            placeholder="Name, e.g. Deploy bot"
            dataQa="outgoing-hook-name"
          />
          <TextInput
            label="Outgoing webhook URL"
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
              <div className="px-3 py-2 text-xs text-muted">No channels to forward.</div>
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
            Create outgoing webhook
          </Button>
        </form>
      </div>
    </div>
  );
}
