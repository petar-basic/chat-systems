import { useState } from 'react';
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

interface Minted {
  label: string;
  value: string;
  note: string;
}

function mintedFrom(secrets: HookSecrets | null | undefined, note: string): Minted | null {
  if (secrets?.incoming_url) {
    return { label: 'Webhook URL', value: secrets.incoming_url, note };
  }
  const secret = (secrets?.config as { secret?: unknown } | undefined)?.secret;
  return typeof secret === 'string'
    ? {
        label: 'Signing secret',
        value: secret,
        note: 'Copy it now: it is not shown again. It signs everything sent from now on.',
      }
    : null;
}

interface Props {
  workspaceId: string;
  app: InstalledApp;
  onRemoved: () => void;
}

export function HookControls({ workspaceId, app, onRemoved }: Props) {
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
                onSuccess: (turnedOn) =>
                  setMinted(
                    mintedFrom(
                      turnedOn.secrets,
                      'Copy it now: it is not shown again. The URL it had before no longer works.',
                    ),
                  ),
              })
            }
            data-qa="app-reinstall"
          >
            Reinstall
          </Button>
        )}
        {app.kind === 'incoming_webhook' && (
          <Button
            size="sm"
            icon={<RefreshCw className="h-4 w-4" />}
            loading={rotate.isPending}
            onClick={() =>
              rotate.mutate(app.id, {
                onSuccess: (made) =>
                  setMinted(
                    mintedFrom(made, 'Copy it now: it is not shown again. The old URL stopped working.'),
                  ),
              })
            }
            data-qa="app-rotate"
          >
            Rotate URL
          </Button>
        )}
        <Button
          size="sm"
          variant="danger"
          icon={<Trash2 className="h-4 w-4" />}
          onClick={() => setConfirmRemove(true)}
          data-qa="app-remove"
        >
          Remove
        </Button>
      </div>
      {minted && (
        <div data-qa="app-rotated-url">
          <CopyField label={minted.label} value={minted.value} />
          <p className="mt-1 text-xs text-muted">{minted.note}</p>
        </div>
      )}
      {failure && (
        <p role="alert" className="text-sm text-danger">
          {toUserMessage(failure, 'The change failed')}
        </p>
      )}
      {confirmRemove && (
        <ConfirmDialog
          title={`Remove ${app.name}?`}
          description="It stops at once. What it already posted stays."
          confirmLabel="Remove"
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
