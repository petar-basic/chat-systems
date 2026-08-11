import { useState, type FormEvent } from 'react';
import { X, Plug, Trash2, Eye, RefreshCw, Copy, Check, ArrowDownToLine, ArrowUpFromLine } from 'lucide-react';
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

function CopyField({ label, value }: { label: string; value: string }) {
  const [copied, setCopied] = useState(false);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      setCopied(false);
    }
  };

  return (
    <div className="mt-2">
      <div className="text-[11px] uppercase tracking-wider text-slate-400 mb-1">{label}</div>
      <div className="flex items-center gap-1.5">
        <code
          data-qa="hook-secret-value"
          className="flex-1 min-w-0 truncate text-xs bg-slate-900/70 border border-slate-700 rounded px-2 py-1.5 text-slate-200"
        >
          {value}
        </code>
        <button
          onClick={copy}
          aria-label={`Copy ${label}`}
          data-qa="hook-secret-copy"
          className="p-1.5 rounded text-slate-400 hover:text-white hover:bg-slate-700 transition cursor-pointer"
        >
          {copied ? <Check className="w-3.5 h-3.5 text-green-400" /> : <Copy className="w-3.5 h-3.5" />}
        </button>
      </div>
    </div>
  );
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
  onDelete: () => void;
}) {
  const [confirmDelete, setConfirmDelete] = useState(false);
  const isIncoming = hook.hook_type === 'incoming_webhook';
  const target = isIncoming ? `#${channelName ?? 'unknown channel'}` : String(hook.config.url ?? '');

  return (
    <div className="px-4 py-3 border-b border-slate-700/40" data-qa="hook-row" data-hook-id={hook.id}>
      <div className="flex items-start gap-2">
        <div className="flex-1 min-w-0">
          <div className="text-sm font-medium text-slate-200 truncate">{hook.name}</div>
          <div className="text-xs text-slate-400 truncate" data-qa="hook-target">
            {target}
          </div>
          {scope && (
            <div className="text-xs text-amber-400/80 truncate" data-qa="hook-scope">
              Forwards {scope}
            </div>
          )}
          {!hook.is_active && (
            <div className="text-xs text-red-400" data-qa="hook-inactive">
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
          className="p-1.5 rounded text-slate-400 hover:text-white hover:bg-slate-700 transition cursor-pointer disabled:opacity-50"
        >
          <Eye className="w-4 h-4" />
        </button>
        <button
          onClick={onRotate}
          disabled={busy}
          aria-label={`Rotate credentials for ${hook.name}`}
          title="Rotate — the old value stops working immediately"
          data-qa="hook-rotate"
          className="p-1.5 rounded text-slate-400 hover:text-amber-400 hover:bg-slate-700 transition cursor-pointer disabled:opacity-50"
        >
          <RefreshCw className="w-4 h-4" />
        </button>
        <button
          onClick={() => setConfirmDelete(true)}
          disabled={busy}
          aria-label={`Delete ${hook.name}`}
          title="Delete"
          data-qa="hook-delete"
          className="p-1.5 rounded text-slate-400 hover:text-red-400 hover:bg-slate-700 transition cursor-pointer disabled:opacity-50"
        >
          <Trash2 className="w-4 h-4" />
        </button>
      </div>

      {confirmDelete && (
        <div className="mt-2 flex items-center gap-2 text-xs">
          <span className="text-red-400">Delete this integration?</span>
          <button
            onClick={() => {
              onDelete();
              setConfirmDelete(false);
            }}
            data-qa="hook-delete-confirm"
            className="px-2 py-1 bg-red-600 hover:bg-red-500 text-white rounded transition cursor-pointer"
          >
            Delete
          </button>
          <button
            onClick={() => setConfirmDelete(false)}
            className="text-slate-400 hover:text-white transition cursor-pointer"
          >
            Cancel
          </button>
        </div>
      )}

      {secrets && (
        <div data-qa="hook-secrets">
          {secrets.incoming_url && <CopyField label="Webhook URL" value={secrets.incoming_url} />}
          {typeof secrets.config.secret === 'string' && (
            <CopyField label="Signing secret" value={secrets.config.secret} />
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
    const ids = Array.isArray(hook.config.channel_ids) ? (hook.config.channel_ids as string[]) : [];
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
    try {
      await deleteHook.mutateAsync(hookId);
      setSecretsByHook((prev) => {
        const next = { ...prev };
        delete next[hookId];
        return next;
      });
    } catch (err: unknown) {
      failWith('Failed to delete the integration')(err);
    }
  };

  const inputClass =
    'w-full px-3 py-2 bg-slate-700/50 border border-slate-600 rounded-lg text-white text-sm placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-purple-500';

  return (
    <div
      className="w-full lg:w-96 max-lg:fixed max-lg:inset-0 max-lg:z-40 bg-slate-800 border-l border-slate-700/50 flex flex-col"
      data-qa="integrations-panel"
    >
      <div className="h-14 px-4 flex items-center justify-between border-b border-slate-700/50 shrink-0">
        <div className="flex items-center gap-2">
          <Plug className="w-4 h-4 text-slate-400" />
          <span className="font-semibold">Integrations</span>
        </div>
        <button onClick={onClose} className="text-slate-400 hover:text-white transition cursor-pointer">
          <X className="w-4 h-4" />
        </button>
      </div>

      <div className="flex-1 overflow-y-auto">
        {error && <div className="px-4 py-2 text-xs text-red-400">{error}</div>}

        <div className="px-4 py-3 flex items-center gap-2 text-xs font-semibold text-slate-400 uppercase tracking-wider">
          <ArrowDownToLine className="w-3.5 h-3.5" />
          Incoming webhooks
        </div>
        <p className="px-4 -mt-1 pb-2 text-xs text-slate-400">
          Post into a channel from anything that can send{' '}
          <code className="text-slate-300">{'{"text":"…"}'}</code>.
        </p>

        {isLoading ? (
          <div className="flex justify-center py-6">
            <div className="w-5 h-5 border-2 border-purple-500/30 border-t-purple-500 rounded-full animate-spin" />
          </div>
        ) : (
          incoming.map((hook) => (
            <HookRow
              key={hook.id}
              hook={hook}
              channelName={channels.find((c) => c.id === hook.config.channel_id)?.name ?? null}
              secrets={secretsByHook[hook.id] ?? null}
              busy={busy}
              onReveal={() => void handleReveal(hook.id)}
              onRotate={() => void handleRotate(hook.id)}
              onDelete={() => void handleDelete(hook.id)}
            />
          ))
        )}

        <form onSubmit={handleCreateIncoming} className="px-4 py-3 space-y-2 border-b border-slate-700/50">
          <input
            type="text"
            value={incomingName}
            onChange={(e) => setIncomingName(e.target.value)}
            placeholder="Name, e.g. CI alerts"
            aria-label="Incoming webhook name"
            data-qa="incoming-hook-name"
            className={inputClass}
          />
          <select
            value={incomingChannel}
            onChange={(e) => setIncomingChannel(e.target.value)}
            aria-label="Incoming webhook channel"
            data-qa="incoming-hook-channel"
            className={inputClass}
          >
            <option value="">Post to channel…</option>
            {postable.map((c) => (
              <option key={c.id} value={c.id}>
                #{c.name}
              </option>
            ))}
          </select>
          <button
            type="submit"
            disabled={busy || !incomingName.trim() || !incomingChannel}
            data-qa="incoming-hook-create"
            className="w-full px-3 py-2 bg-purple-600 hover:bg-purple-500 disabled:bg-purple-600/50 text-white text-sm font-medium rounded-lg transition cursor-pointer disabled:cursor-not-allowed"
          >
            Create incoming webhook
          </button>
        </form>

        <div className="px-4 py-3 flex items-center gap-2 text-xs font-semibold text-slate-400 uppercase tracking-wider">
          <ArrowUpFromLine className="w-3.5 h-3.5" />
          Outgoing webhooks
        </div>
        <p className="px-4 -mt-1 pb-2 text-xs text-slate-400">
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
            onDelete={() => void handleDelete(hook.id)}
          />
        ))}

        <form onSubmit={handleCreateOutgoing} className="px-4 py-3 space-y-2">
          <input
            type="text"
            value={outgoingName}
            onChange={(e) => setOutgoingName(e.target.value)}
            placeholder="Name, e.g. Deploy bot"
            aria-label="Outgoing webhook name"
            data-qa="outgoing-hook-name"
            className={inputClass}
          />
          <input
            type="url"
            value={outgoingUrl}
            onChange={(e) => setOutgoingUrl(e.target.value)}
            placeholder="https://example.com/hooks/chat"
            aria-label="Outgoing webhook URL"
            data-qa="outgoing-hook-url"
            className={inputClass}
          />
          <div
            className="max-h-40 overflow-y-auto rounded-lg border border-slate-600 bg-slate-700/50 divide-y divide-slate-600/50"
            data-qa="outgoing-hook-channels"
          >
            {postable.length === 0 && (
              <div className="px-3 py-2 text-xs text-slate-400">No channels to forward.</div>
            )}
            {postable.map((c) => (
              <label
                key={c.id}
                className="flex items-center gap-2 px-3 py-2 text-sm text-white cursor-pointer hover:bg-slate-700"
              >
                <input
                  type="checkbox"
                  checked={outgoingChannels.includes(c.id)}
                  onChange={() => toggleOutgoingChannel(c.id)}
                  data-qa={`outgoing-hook-channel-${c.id}`}
                  className="accent-purple-500"
                />
                #{c.name}
              </label>
            ))}
          </div>
          <button
            type="submit"
            disabled={busy || !outgoingName.trim() || !outgoingUrl.trim() || outgoingChannels.length === 0}
            data-qa="outgoing-hook-create"
            className="w-full px-3 py-2 bg-purple-600 hover:bg-purple-500 disabled:bg-purple-600/50 text-white text-sm font-medium rounded-lg transition cursor-pointer disabled:cursor-not-allowed"
          >
            Create outgoing webhook
          </button>
        </form>
      </div>
    </div>
  );
}
