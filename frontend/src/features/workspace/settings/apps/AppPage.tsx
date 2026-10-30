import { AlertTriangle, ArrowLeft, Check } from 'lucide-react';
import { Button } from '@/shared/components/Button/Button';
import { formatDateTime } from '@/lib/datetime';
import { displayNameOf } from '@/lib/userHelpers';
import { useApps } from '@/features/workspace/hooks/useApps';
import { useBotApps } from '@/features/workspace/hooks/useBotApps';
import type { AppDirectory, AppRef, InstalledApp } from '@/features/workspace/hooks/useAppDirectory';
import { BotAppControls } from './BotAppControls';
import { ConnectedAppCard } from './ConnectedAppCard';
import { HealthBadge } from './HealthBadge';
import { HookControls } from './HookControls';
import { KINDS } from './kinds';
import { channelText } from './channelText';

interface Props {
  workspaceId: string;
  app: InstalledApp;
  directory: AppDirectory;
  onOpen: (ref: AppRef) => void;
  onBack: () => void;
}

export function AppPage({ workspaceId, app, directory, onOpen, onBack }: Props) {
  const { label, Icon } = KINDS[app.kind];
  const owner = directory.installed.find((other) => other.kind === 'bot_app' && other.id === app.app_id);

  return (
    <article className="space-y-5" data-qa="app-page">
      <Button
        variant="ghost"
        size="sm"
        icon={<ArrowLeft className="h-4 w-4" />}
        onClick={onBack}
        data-qa="app-page-back"
      >
        All apps
      </Button>

      <header className="flex items-start gap-3">
        <Icon className="mt-1 h-5 w-5 shrink-0 text-accent-soft" />
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <h3 className="truncate text-base font-semibold text-fg">{app.name}</h3>
            <HealthBadge health={app.health} />
          </div>
          <p className="text-xs text-muted">
            {label} · added by {displayNameOf(app.added_by_name)} · {formatDateTime(app.added_at)}
          </p>
          {app.description && <p className="mt-1 text-sm text-fg-dim">{app.description}</p>}
          {owner && (
            <button
              onClick={() => onOpen({ kind: 'bot_app', id: owner.id })}
              className="mt-1 text-xs text-accent hover:underline"
              data-qa="app-page-owner"
            >
              Part of {owner.name}
            </button>
          )}
        </div>
      </header>

      {app.health.state !== 'ok' && (
        <p className="flex items-start gap-1.5 text-sm text-danger" data-qa="app-page-health">
          <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
          <span>{healthText(app)}</span>
        </p>
      )}

      <section>
        <h4 className="text-xs font-semibold uppercase tracking-wider text-muted">What it can do</h4>
        {app.permissions.length === 0 ? (
          <p className="mt-2 text-sm text-fg-dim">Nothing beyond being listed here.</p>
        ) : (
          <ul className="mt-2 space-y-1.5" data-qa="app-page-permissions">
            {app.permissions.map((permission) => (
              <li key={permission.key} className="flex items-start gap-2 text-sm text-fg-soft">
                <Check className="mt-0.5 h-4 w-4 shrink-0 text-success" />
                {permission.text}
              </li>
            ))}
          </ul>
        )}
      </section>

      <section>
        <h4 className="text-xs font-semibold uppercase tracking-wider text-muted">Channels</h4>
        <p className="mt-2 text-sm text-fg-dim" data-qa="app-page-channels">
          {channelText(app)}
        </p>
      </section>

      {directory.can_manage ? (
        <Controls workspaceId={workspaceId} app={app} directory={directory} onBack={onBack} />
      ) : (
        <p className="text-xs text-muted" data-qa="app-page-read-only">
          Only a workspace admin can change or remove it.
        </p>
      )}
    </article>
  );
}

function Controls({
  workspaceId,
  app,
  directory,
  onBack,
}: {
  workspaceId: string;
  app: InstalledApp;
  directory: AppDirectory;
  onBack: () => void;
}) {
  const bots = useBotApps(app.kind === 'bot_app' ? workspaceId : undefined);
  const connected = useApps(app.kind === 'connected_app' ? workspaceId : undefined);

  switch (app.kind) {
    case 'bot_app': {
      const bot = bots.data?.data.find((candidate) => candidate.id === app.id);
      return bot ? (
        <BotAppControls key={bot.id} app={bot} scopes={directory.bot_scopes} onRemoved={onBack} />
      ) : null;
    }
    case 'connected_app': {
      const apps = connected.data?.data ?? [];
      const status = apps.find((candidate) => candidate.provider === app.id);
      const permissions = directory.available.find((a) => a.provider === app.id)?.permissions ?? [];
      return status ? (
        <ConnectedAppCard
          workspaceId={workspaceId}
          app={status}
          isAdmin
          setupFirst
          permissions={permissions}
          reusable={
            status.provider === 'confluence'
              ? apps.find((other) => other.provider === 'jira' && other.configured)
              : undefined
          }
        />
      ) : null;
    }
    case 'workflow':
      return (
        <p className="text-xs text-muted" data-qa="app-page-workflow">
          Change or turn it off in Workflows.
        </p>
      );
    default:
      return <HookControls workspaceId={workspaceId} app={app} onRemoved={onBack} />;
  }
}

function healthText(app: InstalledApp) {
  const { state, reason, since } = app.health;
  const when = since ? ` since ${formatDateTime(since)}` : '';
  switch (state) {
    case 'off':
      return 'It is turned off.';
    case 'disabled':
      return `It was turned off${when}${reason ? `: ${reason}` : '.'}`;
    default:
      return `Failing${when}${reason ? `: ${reason}` : '.'}`;
  }
}
