import { useState, type FormEvent } from 'react';
import { RefreshCw, Trash2 } from 'lucide-react';
import { Button } from '@/shared/components/Button/Button';
import { Checkbox } from '@/shared/components/Form/Checkbox';
import { ConfirmDialog } from '@/shared/components/ConfirmDialog/ConfirmDialog';
import { toUserMessage } from '@/lib/errors';
import {
  useBotAppActions,
  type BotApp,
  type BotAppWithToken,
  type BotScope,
} from '@/features/workspace/hooks/useBotApps';
import type { AppPermission } from '@/features/workspace/hooks/useAppDirectory';
import { BotAppEvents } from './BotAppEvents';
import { ConsentDialog } from './ConsentDialog';
import { TokenReveal } from './TokenReveal';

interface Props {
  app: BotApp;
  scopes: AppPermission[];
  onRemoved: () => void;
}

export function BotAppControls({ app, scopes: offered, onRemoved }: Props) {
  const { rotate, remove, setScopes } = useBotAppActions(app.workspace_id);
  const [minted, setMinted] = useState<BotAppWithToken | null>(null);
  const [scopes, setChosen] = useState<BotScope[]>(app.scopes);
  const [consenting, setConsenting] = useState(false);
  const [confirmRemove, setConfirmRemove] = useState(false);

  const added = scopes.filter((scope) => !app.scopes.includes(scope));
  const changed = added.length > 0 || app.scopes.some((scope) => !scopes.includes(scope));

  const toggle = (scope: BotScope, on: boolean) =>
    setChosen((current) => (on ? [...current, scope] : current.filter((s) => s !== scope)));

  const save = () => setScopes.mutateAsync({ appId: app.id, scopes });

  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (added.length > 0) setConsenting(true);
    else void save();
  };

  const failure = rotate.error ?? remove.error ?? setScopes.error;

  return (
    <div className="space-y-4" data-qa="bot-app-controls">
      <div className="flex flex-wrap gap-2">
        <Button
          size="sm"
          icon={<RefreshCw className="h-4 w-4" />}
          loading={rotate.isPending}
          onClick={() => rotate.mutate(app.id, { onSuccess: setMinted })}
          data-qa="bot-app-rotate"
        >
          Rotate token
        </Button>
        <Button
          size="sm"
          variant="danger"
          icon={<Trash2 className="h-4 w-4" />}
          onClick={() => setConfirmRemove(true)}
          data-qa="bot-app-remove"
        >
          Remove
        </Button>
      </div>
      {minted && <TokenReveal name={minted.name} token={minted.token} onDismiss={() => setMinted(null)} />}

      <form onSubmit={submit} className="space-y-2" data-qa="bot-app-scopes-form">
        <h4 className="text-xs font-semibold uppercase tracking-wider text-muted">Scopes</h4>
        {offered.map(({ key, text }) => (
          <Checkbox
            key={key}
            label={key}
            description={text}
            checked={scopes.includes(key as BotScope)}
            onChange={(on) => toggle(key as BotScope, on)}
            dataQa={`bot-app-scope-edit-${key}`}
          />
        ))}
        <Button
          type="submit"
          size="sm"
          variant="primary"
          disabled={!changed}
          loading={setScopes.isPending}
          data-qa="bot-app-scopes-save"
        >
          Save scopes
        </Button>
      </form>

      <BotAppEvents app={app} />

      {failure && (
        <p role="alert" className="text-sm text-danger">
          {toUserMessage(failure, 'The app change failed')}
        </p>
      )}
      {consenting && (
        <ConsentDialog
          title={`Give ${app.name} more access?`}
          lead="Its token keeps working, and from now on the app will also be able to:"
          permissions={offered.filter((p) => added.includes(p.key as BotScope))}
          confirmLabel="Agree and save"
          onConfirm={async () => {
            await save();
          }}
          onClose={() => setConsenting(false)}
        />
      )}
      {confirmRemove && (
        <ConfirmDialog
          title={`Remove ${app.name}?`}
          description="Its token stops working at once, and its slash commands and webhooks are removed with it. What it posted stays."
          confirmLabel="Remove app"
          onConfirm={async () => {
            await remove.mutateAsync(app.id);
            onRemoved();
          }}
          onClose={() => setConfirmRemove(false)}
          dataQa="bot-app-remove-dialog"
        />
      )}
    </div>
  );
}
