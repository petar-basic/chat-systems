import { useState, type FormEvent } from 'react';
import { Bot, RefreshCw, Trash2 } from 'lucide-react';
import { Button } from '@/shared/components/Button/Button';
import { TextInput } from '@/shared/components/Form/TextInput';
import { Checkbox } from '@/shared/components/Form/Checkbox';
import { CopyField } from '@/shared/components/CopyField/CopyField';
import { ConfirmDialog } from '@/shared/components/ConfirmDialog/ConfirmDialog';
import { toUserMessage } from '@/lib/errors';
import {
  useBotAppActions,
  useBotApps,
  type BotApp,
  type BotAppWithToken,
  type BotScope,
} from '@/features/workspace/hooks/useBotApps';

const SCOPES: { scope: BotScope; label: string }[] = [
  { scope: 'chat:write', label: 'Post, edit and delete its own messages where it is a member' },
  { scope: 'chat:write.public', label: 'Post in public channels it has not been added to' },
  { scope: 'channels:read', label: 'List the channels it is in' },
  { scope: 'channels:history', label: 'Read the public channels it is in' },
  { scope: 'groups:history', label: 'Read the private channels and DMs it is in' },
  { scope: 'im:write', label: 'Start direct messages' },
  { scope: 'reactions:write', label: 'Add and remove reactions' },
  { scope: 'files:write', label: 'Upload files' },
  { scope: 'users:read', label: 'Look people up in the workspace' },
  { scope: 'commands', label: 'Own slash commands' },
];

interface Props {
  workspaceId: string;
  isAdmin: boolean;
}

function BotAppRow({
  app,
  isAdmin,
  busy,
  onRotate,
  onRemove,
}: {
  app: BotApp;
  isAdmin: boolean;
  busy: boolean;
  onRotate: () => void;
  onRemove: () => Promise<unknown>;
}) {
  const [confirmRemove, setConfirmRemove] = useState(false);

  return (
    <div className="flex items-start gap-3 rounded-lg border border-line/50 p-3" data-qa="bot-app-row">
      <Bot className="mt-0.5 h-4 w-4 shrink-0 text-accent-soft" />
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-1.5 text-sm text-fg-soft">
          <span className="truncate font-medium">{app.name}</span>
          <span className="px-1 py-0.5 text-[10px] font-semibold uppercase tracking-wide bg-raised text-fg-dim rounded">
            App
          </span>
        </div>
        {app.description && <p className="text-xs text-muted">{app.description}</p>}
        <div className="mt-1 flex flex-wrap gap-1" data-qa="bot-app-scopes">
          {app.scopes.length === 0 ? (
            <span className="text-xs text-muted">No scopes</span>
          ) : (
            app.scopes.map((scope) => (
              <code key={scope} className="rounded bg-raised px-1 text-[11px] text-fg-dim">
                {scope}
              </code>
            ))
          )}
        </div>
      </div>
      {isAdmin && (
        <div className="flex shrink-0 items-center gap-1">
          <button
            onClick={onRotate}
            disabled={busy}
            aria-label={`Make a new token for ${app.name}`}
            title="New token"
            data-qa="bot-app-rotate"
            className="p-1.5 rounded text-muted hover:text-fg hover:bg-raised transition cursor-pointer disabled:opacity-50"
          >
            <RefreshCw className="h-4 w-4" />
          </button>
          <button
            onClick={() => setConfirmRemove(true)}
            disabled={busy}
            aria-label={`Remove ${app.name}`}
            title="Remove"
            data-qa="bot-app-remove"
            className="p-1.5 rounded text-muted hover:text-danger hover:bg-raised transition cursor-pointer disabled:opacity-50"
          >
            <Trash2 className="h-4 w-4" />
          </button>
        </div>
      )}
      {confirmRemove && (
        <ConfirmDialog
          title={`Remove ${app.name}?`}
          description="Its token stops working at once, and its slash commands and webhooks are removed with it. What it posted stays."
          confirmLabel="Remove app"
          onConfirm={async () => {
            await onRemove();
          }}
          onClose={() => setConfirmRemove(false)}
          dataQa="bot-app-remove-dialog"
        />
      )}
    </div>
  );
}

export function BotAppsSection({ workspaceId, isAdmin }: Props) {
  const { data, isLoading, isError } = useBotApps(workspaceId);
  const { create, rotate, remove } = useBotAppActions(workspaceId);
  const apps = data?.data ?? [];
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [scopes, setScopes] = useState<BotScope[]>(['chat:write']);
  const [minted, setMinted] = useState<BotAppWithToken | null>(null);

  const submit = (e: FormEvent) => {
    e.preventDefault();
    create.mutate(
      { name: name.trim(), description: description.trim(), scopes },
      {
        onSuccess: (app) => {
          setMinted(app);
          setName('');
          setDescription('');
          setScopes(['chat:write']);
        },
      },
    );
  };

  const toggle = (scope: BotScope, on: boolean) =>
    setScopes((current) => (on ? [...current, scope] : current.filter((s) => s !== scope)));

  const failure = create.error ?? rotate.error ?? remove.error;

  return (
    <section className="space-y-3" data-qa="bot-apps-section">
      <div>
        <h3 className="text-sm font-semibold text-fg-soft">Built for this workspace</h3>
        <p className="text-xs text-muted">
          An app has a bot user and a token of its own. It reads and writes only in the channels it is added
          to, with the scopes given here. The API it calls is described in the bot API guide.
        </p>
      </div>
      {isLoading && <p className="text-sm text-muted">Loading…</p>}
      {isError && <p className="text-sm text-danger">Couldn't load the apps.</p>}
      {!isLoading && apps.length === 0 && <p className="text-sm text-muted">No apps yet.</p>}
      {apps.map((app) => (
        <BotAppRow
          key={app.id}
          app={app}
          isAdmin={isAdmin}
          busy={rotate.isPending || remove.isPending}
          onRotate={() => rotate.mutate(app.id, { onSuccess: setMinted })}
          onRemove={() => remove.mutateAsync(app.id)}
        />
      ))}
      {minted && (
        <div className="rounded-lg border border-warning/40 bg-warning/5 p-3" data-qa="bot-app-token">
          <p className="text-sm text-fg-soft">
            Copy {minted.name}'s token now. It is not shown again, and nobody can read it back later.
          </p>
          <CopyField
            label="Bot token"
            value={minted.token}
            valueQa="bot-app-token-value"
            copyQa="bot-app-token-copy"
          />
          <button
            onClick={() => setMinted(null)}
            data-qa="bot-app-token-dismiss"
            className="mt-2 text-xs text-muted hover:text-fg transition cursor-pointer"
          >
            I have stored it
          </button>
        </div>
      )}
      {isAdmin && (
        <form
          onSubmit={submit}
          className="space-y-3 rounded-lg border border-line/50 p-3"
          data-qa="bot-app-form"
        >
          <TextInput
            label="App name"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="Deploy bot"
            dataQa="bot-app-name"
          />
          <TextInput
            label="Description"
            optional
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            dataQa="bot-app-description"
          />
          <fieldset className="space-y-2">
            <legend className="mb-1 text-xs font-medium text-fg-dim">Scopes</legend>
            {SCOPES.map(({ scope, label }) => (
              <Checkbox
                key={scope}
                label={scope}
                description={label}
                checked={scopes.includes(scope)}
                onChange={(on) => toggle(scope, on)}
                dataQa={`bot-app-scope-${scope}`}
              />
            ))}
          </fieldset>
          <Button
            type="submit"
            variant="primary"
            loading={create.isPending}
            disabled={!name.trim()}
            data-qa="bot-app-create"
          >
            Create app
          </Button>
        </form>
      )}
      {failure && (
        <p role="alert" className="text-sm text-danger">
          {toUserMessage(failure, 'The app change failed')}
        </p>
      )}
    </section>
  );
}
