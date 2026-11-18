import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { RefreshCw, RotateCcw, Trash2 } from 'lucide-react';
import { Button } from '@/shared/components/Button/Button';
import { ConfirmDialog } from '@/shared/components/ConfirmDialog/ConfirmDialog';
import { CopyField } from '@/shared/components/CopyField/CopyField';
import { toUserMessage } from '@/lib/errors';
import { useWorkspaceStore } from '@/stores/workspace';
import {
  useDeleteHook,
  useEnableHook,
  useRotateHook,
  type HookSecrets,
} from '@/features/workspace/hooks/useHooks';
import type { InstalledApp } from '@/features/workspace/hooks/useAppDirectory';

type UrlNote = 'reinstallNote' | 'rotateNote';

interface Minted {
  label: 'webhookUrl' | 'signingSecret';
  value: string;
  note: UrlNote | 'secretNote';
}

function mintedFrom(secrets: HookSecrets | null | undefined, note: UrlNote): Minted | null {
  if (secrets?.incoming_url) {
    return { label: 'webhookUrl', value: secrets.incoming_url, note };
  }
  const secret = (secrets?.config as { secret?: unknown } | undefined)?.secret;
  return typeof secret === 'string' ? { label: 'signingSecret', value: secret, note: 'secretNote' } : null;
}

interface Props {
  workspaceId: string;
  app: InstalledApp;
  onRemoved: () => void;
}

export function HookControls({ workspaceId, app, onRemoved }: Props) {
  const { t } = useTranslation('workspace');
  const instanceUrl = useWorkspaceStore((s) => s.currentWorkspace?.instanceUrl);
  const enable = useEnableHook(workspaceId, instanceUrl);
  const rotate = useRotateHook(instanceUrl);
  const remove = useDeleteHook(workspaceId, instanceUrl);
  const [minted, setMinted] = useState<Minted | null>(null);
  const [confirmRemove, setConfirmRemove] = useState(false);
  const failure = enable.error ?? rotate.error ?? remove.error;

  return (
    <div className="space-y-3" data-qa="hook-controls">
      <div className="flex flex-wrap gap-2">
        {app.health.state !== 'ok' && (
          <Button
            size="sm"
            variant="primary"
            icon={<RotateCcw className="h-4 w-4" />}
            loading={enable.isPending}
            onClick={() =>
              enable.mutate(app.id, {
                onSuccess: (turnedOn) => setMinted(mintedFrom(turnedOn.secrets, 'reinstallNote')),
              })
            }
            data-qa="app-reinstall"
          >
            {t('settings.apps.hook.reinstall')}
          </Button>
        )}
        {app.kind === 'incoming_webhook' && (
          <Button
            size="sm"
            icon={<RefreshCw className="h-4 w-4" />}
            loading={rotate.isPending}
            onClick={() =>
              rotate.mutate(app.id, {
                onSuccess: (made) => setMinted(mintedFrom(made, 'rotateNote')),
              })
            }
            data-qa="app-rotate"
          >
            {t('settings.apps.hook.rotate')}
          </Button>
        )}
        <Button
          size="sm"
          variant="danger"
          icon={<Trash2 className="h-4 w-4" />}
          onClick={() => setConfirmRemove(true)}
          data-qa="app-remove"
        >
          {t('settings.apps.hook.remove')}
        </Button>
      </div>
      {minted && (
        <div data-qa="app-rotated-url">
          <CopyField label={t(`settings.apps.hook.${minted.label}`)} value={minted.value} />
          <p className="mt-1 text-xs text-muted">{t(`settings.apps.hook.${minted.note}`)}</p>
        </div>
      )}
      {failure && (
        <p role="alert" className="text-sm text-danger">
          {toUserMessage(failure, t('settings.apps.changeFailed'))}
        </p>
      )}
      {confirmRemove && (
        <ConfirmDialog
          title={t('settings.apps.hook.removeTitle', { name: app.name })}
          description={t('settings.apps.hook.removeDescription')}
          confirmLabel={t('settings.apps.hook.remove')}
          onConfirm={async () => {
            await remove.mutateAsync(app.id);
            onRemoved();
          }}
          onClose={() => setConfirmRemove(false)}
          dataQa="app-remove-dialog"
        />
      )}
    </div>
  );
}
