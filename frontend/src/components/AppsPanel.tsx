import { useState, type FormEvent } from 'react';
import { ExternalLink, LayoutGrid, X } from 'lucide-react';
import { CopyField } from '@/shared/components/CopyField/CopyField';
import { useApps, useAppActions, type AppStatus } from '@/hooks/queries/useApps';

interface Props {
  workspaceId: string;
  isAdmin: boolean;
  onClose: () => void;
}

export default function AppsPanel({ workspaceId, isAdmin, onClose }: Props) {
  const { data, isLoading, isError } = useApps(workspaceId);
  const apps = data?.data ?? [];

  return (
    <div className="w-80 bg-surface/80 border-l border-line/50 flex flex-col h-full" data-qa="apps-panel">
      <div className="h-14 px-4 flex items-center justify-between border-b border-line/50 shrink-0">
        <h2 className="font-semibold text-fg flex items-center gap-2">
          <LayoutGrid className="w-4 h-4" /> Apps
        </h2>
        <button
          onClick={onClose}
          aria-label="Close"
          className="text-muted hover:text-fg transition cursor-pointer"
        >
          <X className="w-5 h-5" />
        </button>
      </div>
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        <p className="text-xs text-muted">
          Connected apps turn links into cards with live details. Each person connects their own account, so a
          card only ever shows what the person who posted the link can see.
        </p>
        {isLoading && <p className="text-sm text-muted">Loading…</p>}
        {isError && <p className="text-sm text-danger">Couldn't load the apps.</p>}
        {apps.map((app) => (
          <AppCardSection key={app.provider} workspaceId={workspaceId} app={app} isAdmin={isAdmin} />
        ))}
      </div>
    </div>
  );
}

function AppCardSection({
  workspaceId,
  app,
  isAdmin,
}: {
  workspaceId: string;
  app: AppStatus;
  isAdmin: boolean;
}) {
  const actions = useAppActions(workspaceId);
  const [setupOpen, setSetupOpen] = useState(!app.configured && isAdmin);
  const status = !app.configured ? 'Not set up' : app.enabled ? 'On' : 'Off';

  return (
    <section className="rounded-lg border border-line/60 p-3" data-qa={`app-${app.provider}`}>
      <div className="flex items-center justify-between">
        <h3 className="font-semibold text-sm text-fg">{app.name}</h3>
        <span className="text-xs text-muted">{status}</span>
      </div>

      {app.configured && app.enabled && (
        <div className="mt-2 text-sm">
          {app.connected_account ? (
            <div className="flex items-center justify-between gap-2">
              <span className="text-fg-dim truncate" title={app.connected_account}>
                Connected as {app.connected_account}
              </span>
              <button
                onClick={() => actions.disconnect.mutate(app.provider)}
                disabled={actions.disconnect.isPending}
                className="text-xs text-danger hover:underline shrink-0"
              >
                Disconnect
              </button>
            </div>
          ) : (
            <button
              onClick={() => actions.connect.mutate({ provider: app.provider })}
              disabled={actions.connect.isPending}
              data-qa="app-connect"
              className="w-full px-3 py-2 text-sm bg-purple-600 hover:bg-purple-500 text-white rounded-lg transition disabled:opacity-50"
            >
              Connect my account
            </button>
          )}
        </div>
      )}
      {app.configured && !app.enabled && !isAdmin && (
        <p className="mt-2 text-xs text-muted">An admin has turned this app off.</p>
      )}
      {!app.configured && !isAdmin && (
        <p className="mt-2 text-xs text-muted">Ask a workspace admin to set this app up.</p>
      )}

      {isAdmin && (
        <div className="mt-3 border-t border-line/50 pt-2">
          <button
            onClick={() => setSetupOpen((open) => !open)}
            className="text-xs text-accent hover:underline"
          >
            {setupOpen ? 'Hide setup' : app.configured ? 'Edit setup' : 'Set up'}
          </button>
          {setupOpen && (
            <AppSetupForm workspaceId={workspaceId} app={app} onDone={() => setSetupOpen(false)} />
          )}
        </div>
      )}
    </section>
  );
}

function AppSetupForm({
  workspaceId,
  app,
  onDone,
}: {
  workspaceId: string;
  app: AppStatus;
  onDone: () => void;
}) {
  const actions = useAppActions(workspaceId);
  const [clientId, setClientId] = useState(app.client_id ?? '');
  const [clientSecret, setClientSecret] = useState('');
  const [enabled, setEnabled] = useState(app.configured ? app.enabled : true);
  const [confirmRemove, setConfirmRemove] = useState(false);

  const save = (event: FormEvent) => {
    event.preventDefault();
    actions.configure.mutate(
      { provider: app.provider, clientId: clientId.trim(), clientSecret: clientSecret.trim(), enabled },
      { onSuccess: () => setClientSecret('') },
    );
  };

  return (
    <form onSubmit={save} className="mt-2 space-y-3" data-qa="app-setup-form">
      <ol className="list-decimal pl-4 space-y-1 text-xs text-fg-dim">
        {app.setup_steps.map((step) => (
          <li key={step}>{step}</li>
        ))}
      </ol>
      <a
        href={app.console_url}
        target="_blank"
        rel="noopener noreferrer"
        className="inline-flex items-center gap-1 text-xs text-accent hover:underline"
      >
        Open the developer console <ExternalLink className="w-3 h-3" />
      </a>
      <CopyField label="Callback URL" value={app.redirect_url} />
      <CopyField label="Scopes" value={app.scopes.join(' ')} />

      <label className="block text-xs text-fg-dim">
        Client ID
        <input
          value={clientId}
          onChange={(e) => setClientId(e.target.value)}
          required
          className="mt-1 w-full px-2 py-1.5 text-sm bg-raised/50 border border-line-strong rounded text-fg focus:outline-none focus:ring-2 focus:ring-purple-500"
        />
      </label>
      <label className="block text-xs text-fg-dim">
        Secret
        <input
          type="password"
          value={clientSecret}
          onChange={(e) => setClientSecret(e.target.value)}
          required={!app.configured}
          placeholder={app.configured ? 'Stored — leave blank to keep it' : ''}
          autoComplete="off"
          className="mt-1 w-full px-2 py-1.5 text-sm bg-raised/50 border border-line-strong rounded text-fg focus:outline-none focus:ring-2 focus:ring-purple-500"
        />
      </label>
      <label className="flex items-center gap-2 text-xs text-fg-dim">
        <input type="checkbox" checked={enabled} onChange={(e) => setEnabled(e.target.checked)} />
        On for this workspace
      </label>

      <div className="flex items-center gap-2">
        <button
          type="submit"
          disabled={actions.configure.isPending}
          className="px-3 py-1.5 text-sm bg-purple-600 hover:bg-purple-500 text-white rounded-lg disabled:opacity-50"
        >
          Save
        </button>
        {app.configured &&
          (confirmRemove ? (
            <button
              type="button"
              onClick={() => actions.remove.mutate(app.provider, { onSuccess: onDone })}
              className="text-xs text-danger hover:underline"
            >
              Remove, and disconnect everyone
            </button>
          ) : (
            <button
              type="button"
              onClick={() => setConfirmRemove(true)}
              className="text-xs text-muted hover:text-danger"
            >
              Remove app
            </button>
          ))}
      </div>
    </form>
  );
}
