import { useState, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { RefreshCw, RotateCcw, Trash2 } from 'lucide-react';
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
  const { t } = useTranslation('workspace');
  const { rotate, enable, remove, setScopes } = useBotAppActions(app.workspace_id);
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

  const stopped = !!app.disabled_at;
  const failure = rotate.error ?? enable.error ?? remove.error ?? setScopes.error;

  return (
    <div className="space-y-4" data-qa="bot-app-controls">
      <div className="flex flex-wrap gap-2">
        {stopped ? (
          <Button
            size="sm"
            variant="primary"
            icon={<RotateCcw className="h-4 w-4" />}
            loading={enable.isPending}
            onClick={() => enable.mutate(app.id, { onSuccess: setMinted })}
            data-qa="bot-app-enable"
          >
            {t('settings.apps.bot.turnOn')}
          </Button>
        ) : (
          <Button
            size="sm"
            icon={<RefreshCw className="h-4 w-4" />}
            loading={rotate.isPending}
            onClick={() => rotate.mutate(app.id, { onSuccess: setMinted })}
            data-qa="bot-app-rotate"
          >
            {t('settings.apps.bot.rotate')}
          </Button>
        )}
        <Button
          size="sm"
          variant="danger"
          icon={<Trash2 className="h-4 w-4" />}
          onClick={() => setConfirmRemove(true)}
          data-qa="bot-app-remove"
        >
          {t('settings.apps.bot.remove')}
        </Button>
      </div>
      {stopped && !minted && (
        <p className="text-xs text-muted" data-qa="bot-app-stopped">
          {t('settings.apps.bot.stopped')}
        </p>
      )}
      {minted && <TokenReveal name={minted.name} token={minted.token} onDismiss={() => setMinted(null)} />}

      <form onSubmit={submit} className="space-y-2" data-qa="bot-app-scopes-form">
        <h4 className="text-xs font-semibold uppercase tracking-wider text-muted">
          {t('settings.apps.bot.scopes')}
        </h4>
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
          {t('settings.apps.bot.saveScopes')}
        </Button>
      </form>

      <BotAppEvents app={app} />

      {failure && (
        <p role="alert" className="text-sm text-danger">
          {toUserMessage(failure, t('settings.apps.bot.failed'))}
        </p>
      )}
      {consenting && (
        <ConsentDialog
          title={t('settings.apps.bot.moreTitle', { name: app.name })}
          lead={t('settings.apps.bot.moreLead')}
          permissions={offered.filter((p) => added.includes(p.key as BotScope))}
          confirmLabel={t('settings.apps.bot.moreConfirm')}
          onConfirm={async () => {
            await save();
          }}
          onClose={() => setConsenting(false)}
        />
      )}
      {confirmRemove && (
        <ConfirmDialog
          title={t('settings.apps.bot.removeTitle', { name: app.name })}
          description={t('settings.apps.bot.removeDescription')}
          confirmLabel={t('settings.apps.bot.removeConfirm')}
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
