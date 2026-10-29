import { useState, type FormEvent } from 'react';
import { ExternalLink } from 'lucide-react';
import { Button } from '@/shared/components/Button/Button';
import { CopyField } from '@/shared/components/CopyField/CopyField';
import { AppIcon } from '@/shared/components/AppIcon/AppIcon';
import { TextInput } from '@/shared/components/Form/TextInput';
import { Checkbox } from '@/shared/components/Form/Checkbox';
import { useAppActions, type AppStatus } from '@/features/workspace/hooks/useApps';
import type { AppPermission } from '@/features/workspace/hooks/useAppDirectory';
import { ConsentDialog } from './ConsentDialog';

export function ConnectedAppCard({
  workspaceId,
  app,
  isAdmin,
  reusable,
  permissions,
}: {
  workspaceId: string;
  app: AppStatus;
  isAdmin: boolean;
  reusable?: AppStatus;
  permissions: AppPermission[];
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
              permissions={permissions}
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
  permissions,
  onDone,
}: {
  workspaceId: string;
  app: AppStatus;
  reusable?: AppStatus;
  permissions: AppPermission[];
  onDone: () => void;
}) {
  const actions = useAppActions(workspaceId);
  const [clientId, setClientId] = useState(app.client_id ?? '');
  const [clientSecret, setClientSecret] = useState('');
  const [enabled, setEnabled] = useState(app.configured ? app.enabled : true);
  const [confirmRemove, setConfirmRemove] = useState(false);
  const [reuse, setReuse] = useState(!!reusable && (!app.configured || app.client_id === reusable.client_id));

  const [consenting, setConsenting] = useState(false);

  const store = () =>
    actions.configure.mutateAsync(
      reuse && reusable
        ? { provider: app.provider, clientId: '', clientSecret: '', enabled, reuseFrom: reusable.provider }
        : { provider: app.provider, clientId: clientId.trim(), clientSecret: clientSecret.trim(), enabled },
      { onSuccess: () => setClientSecret('') },
    );

  const save = (event: FormEvent) => {
    event.preventDefault();
    if (app.configured) void store().catch(() => undefined);
    else setConsenting(true);
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
      {consenting && (
        <ConsentDialog
          title={`Install ${app.name}?`}
          lead="Nobody can connect an account until you agree. For each person who connects, it will be able to:"
          permissions={permissions}
          confirmLabel="Agree and install"
          onConfirm={async () => {
            await store();
          }}
          onClose={() => setConsenting(false)}
        />
      )}
    </form>
  );
}
