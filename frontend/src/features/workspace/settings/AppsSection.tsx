import { useState, type FormEvent } from 'react';
import { Button } from '@/shared/components/Button/Button';
import { ExternalLink } from 'lucide-react';
import { CopyField } from '@/shared/components/CopyField/CopyField';
import { AppIcon } from '@/shared/components/AppIcon/AppIcon';
import { useApps, useAppActions, type AppStatus } from '@/features/workspace/hooks/useApps';
import { TextInput } from '@/shared/components/Form/TextInput';
import { Checkbox } from '@/shared/components/Form/Checkbox';

interface Props {
  workspaceId: string;
  isAdmin: boolean;
}

export function AppsSection({ workspaceId, isAdmin }: Props) {
  const { data, isLoading, isError } = useApps(workspaceId);
  const apps = data?.data ?? [];

  return (
    <div className="-mx-4 flex flex-col" data-qa="apps-panel">
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        <p className="text-xs text-muted">
          Connected apps turn links into cards with live details. Each person connects their own account, so a
          card only ever shows what the person who posted the link can see.
        </p>
        {isLoading && <p className="text-sm text-muted">Loading…</p>}
        {isError && <p className="text-sm text-danger">Couldn't load the apps.</p>}
        {apps.map((app) => (
          <AppCardSection
            key={app.provider}
            workspaceId={workspaceId}
            app={app}
            isAdmin={isAdmin}
            reusable={
              app.provider === 'confluence'
                ? apps.find((other) => other.provider === 'jira' && other.configured)
                : undefined
            }
          />
        ))}
      </div>
    </div>
  );
}

function AppCardSection({
  workspaceId,
  app,
  isAdmin,
  reusable,
}: {
  workspaceId: string;
  app: AppStatus;
  isAdmin: boolean;
  reusable?: AppStatus;
}) {
  const actions = useAppActions(workspaceId);
  const [setupOpen, setSetupOpen] = useState(!app.configured && isAdmin);
  const usable = app.configured && app.enabled;
  const badge = app.connected_account
    ? 'Connected'
    : !app.configured
      ? 'Not set up'
      : app.enabled
        ? null
        : 'Off';

  return (
    <section className="rounded-lg border border-line/60 p-3" data-qa={`app-${app.provider}`}>
      <div className="flex items-start gap-3">
        <AppIcon provider={app.provider} size="md" />
        <div className="min-w-0 flex-1">
          <div className="flex items-center justify-between gap-2">
            <h3 className="font-semibold text-sm text-fg">{app.name}</h3>
            {badge && (
              <span
                className={`shrink-0 rounded px-1.5 py-0.5 text-[11px] font-medium ${
                  app.connected_account ? 'bg-green-500/15 text-success' : 'bg-raised text-muted'
                }`}
              >
                {badge}
              </span>
            )}
          </div>
          <p className="text-xs text-muted">{app.summary}</p>
        </div>
      </div>

      {usable && (
        <div className="mt-3 space-y-2 text-sm">
          {app.connected_account ? (
            <div className="flex items-center justify-between gap-2">
              <span className="text-fg-dim truncate" title={app.connected_account}>
                {app.connected_account}
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
            <Button
              variant="primary"
              fullWidth
              onClick={() => actions.connect.mutate({ provider: app.provider })}
              disabled={actions.connect.isPending}
              data-qa="app-connect"
            >
              Connect my account
            </Button>
          )}
          {app.prompts_off && !app.connected_account && (
            <div
              className="flex items-center justify-between gap-2 text-xs text-muted"
              data-qa="app-prompts-off"
            >
              <span>Prompts off</span>
              <button
                onClick={() => actions.prompts.mutate({ provider: app.provider, off: false })}
                disabled={actions.prompts.isPending}
                className="text-accent hover:underline"
              >
                Turn on
              </button>
            </div>
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
            <AppSetupForm
              workspaceId={workspaceId}
              app={app}
              reusable={reusable}
              onDone={() => setSetupOpen(false)}
            />
          )}
        </div>
      )}
    </section>
  );
}

function AppSetupForm({
  workspaceId,
  app,
  reusable,
  onDone,
}: {
  workspaceId: string;
  app: AppStatus;
  reusable?: AppStatus;
  onDone: () => void;
}) {
  const actions = useAppActions(workspaceId);
  const [clientId, setClientId] = useState(app.client_id ?? '');
  const [clientSecret, setClientSecret] = useState('');
  const [enabled, setEnabled] = useState(app.configured ? app.enabled : true);
  const [confirmRemove, setConfirmRemove] = useState(false);
  const [reuse, setReuse] = useState(!!reusable && (!app.configured || app.client_id === reusable.client_id));

  const save = (event: FormEvent) => {
    event.preventDefault();
    actions.configure.mutate(
      reuse && reusable
        ? { provider: app.provider, clientId: '', clientSecret: '', enabled, reuseFrom: reusable.provider }
        : { provider: app.provider, clientId: clientId.trim(), clientSecret: clientSecret.trim(), enabled },
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
      {app.scopes.length > 0 && <CopyField label="Scopes" value={app.scopes.join(' ')} />}

      {reusable && (
        <Checkbox
          label={`Use the ${reusable.name} app`}
          description={`Add the ${app.name} API to it first.`}
          checked={reuse}
          onChange={setReuse}
          dataQa="app-reuse"
        />
      )}
      {!reuse && (
        <>
          <TextInput
            label="Client ID"
            compact
            value={clientId}
            onChange={(e) => setClientId(e.target.value)}
            required
          />
          <TextInput
            label="Secret"
            compact
            type="password"
            value={clientSecret}
            onChange={(e) => setClientSecret(e.target.value)}
            required={!app.configured}
            placeholder={app.configured ? 'Stored — leave blank to keep it' : ''}
            autoComplete="off"
          />
        </>
      )}
      <Checkbox label="On for this workspace" checked={enabled} onChange={setEnabled} />

      <div className="flex items-center gap-2">
        <Button variant="primary" type="submit" disabled={actions.configure.isPending}>
          Save
        </Button>
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
