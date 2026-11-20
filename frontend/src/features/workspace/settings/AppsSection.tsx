import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ChevronRight, Hammer } from 'lucide-react';
import { Button } from '@/shared/components/Button/Button';
import { QueryState } from '@/shared/components/QueryState/QueryState';
import { displayNameOf } from '@/lib/userHelpers';
import { useApps, type AppStatus } from '@/features/workspace/hooks/useApps';
import {
  sameApp,
  useAppDirectory,
  type AppDirectory,
  type AppRef,
  type InstalledApp,
} from '@/features/workspace/hooks/useAppDirectory';
import { AppPage } from './apps/AppPage';
import { channelText } from './apps/channelText';
import { BuildAnApp } from './apps/BuildAnApp';
import { ConnectedAppCard } from './apps/ConnectedAppCard';
import { HealthBadge } from './apps/HealthBadge';
import { RequestApp } from './apps/RequestApp';
import { KINDS } from './apps/kinds';

interface Props {
  workspaceId: string;
  initialApp?: AppRef | null;
}

const REUSES: Partial<Record<AppStatus['provider'], AppStatus['provider']>> = {
  confluence: 'jira',
  google_calendar: 'google_drive',
};

export function AppsSection({ workspaceId, initialApp = null }: Props) {
  const { t } = useTranslation('workspace');
  const directory = useAppDirectory(workspaceId);
  const apps = useApps(workspaceId);
  const [open, setOpen] = useState<AppRef | null>(initialApp);
  const data = directory.data;
  const selected = open && data?.installed.find((app) => sameApp(app, open));

  return (
    <div className="-mx-4 flex flex-col" data-qa="apps-panel">
      <div className="flex-1 overflow-y-auto p-4 space-y-6">
        <QueryState
          isLoading={directory.isLoading || apps.isLoading}
          isError={directory.isError || apps.isError}
          onRetry={() => {
            void directory.refetch();
            void apps.refetch();
          }}
          errorMessage={t('settings.apps.loadFailed')}
        >
          {data &&
            (selected ? (
              <AppPage
                workspaceId={workspaceId}
                app={selected}
                directory={data}
                onOpen={setOpen}
                onBack={() => setOpen(null)}
              />
            ) : (
              <Directory
                workspaceId={workspaceId}
                directory={data}
                statuses={apps.data?.data ?? []}
                missing={!!open}
                onOpen={setOpen}
              />
            ))}
        </QueryState>
      </div>
    </div>
  );
}

function Directory({
  workspaceId,
  directory,
  statuses,
  missing,
  onOpen,
}: {
  workspaceId: string;
  directory: AppDirectory;
  statuses: AppStatus[];
  missing: boolean;
  onOpen: (ref: AppRef) => void;
}) {
  const { t } = useTranslation('workspace');
  const isAdmin = directory.can_manage;
  const [building, setBuilding] = useState(false);

  return (
    <>
      {missing && (
        <p className="text-sm text-muted" data-qa="app-directory-missing">
          {t('settings.apps.missing')}
        </p>
      )}
      <section className="space-y-2" data-qa="app-directory-installed">
        <div>
          <h3 className="text-sm font-semibold text-fg-soft">{t('settings.apps.installed')}</h3>
          <p className="text-xs text-muted">{t('settings.apps.installedHint')}</p>
        </div>
        {directory.installed.length === 0 ? (
          <p className="text-sm text-muted">{t('settings.apps.nothingInstalled')}</p>
        ) : (
          directory.installed.map((app) => (
            <InstalledRow key={`${app.kind}:${app.id}`} app={app} onOpen={() => onOpen(app)} />
          ))
        )}
      </section>

      <section className="space-y-3" data-qa="app-directory-available">
        <div>
          <h3 className="text-sm font-semibold text-fg-soft">{t('settings.apps.available')}</h3>
          <p className="text-xs text-muted">{t('settings.apps.availableHint')}</p>
        </div>
        {statuses.map((status) => (
          <ConnectedAppCard
            key={status.provider}
            workspaceId={workspaceId}
            app={status}
            isAdmin={isAdmin}
            permissions={directory.available.find((a) => a.provider === status.provider)?.permissions ?? []}
            reusable={statuses.find(
              (other) => other.provider === REUSES[status.provider] && other.configured,
            )}
          />
        ))}
        <section className="rounded-lg border border-line/60 p-3" data-qa="app-directory-build">
          <div className="flex items-start gap-3">
            <Hammer className="mt-0.5 h-5 w-5 shrink-0 text-accent-soft" />
            <div className="min-w-0 flex-1">
              <h3 className="text-sm font-semibold text-fg">{t('settings.apps.build')}</h3>
              <p className="text-xs text-muted">{t('settings.apps.buildHint')}</p>
            </div>
            {isAdmin && !building && (
              <Button size="sm" onClick={() => setBuilding(true)} data-qa="app-directory-build-open">
                {t('settings.apps.buildOpen')}
              </Button>
            )}
          </div>
          {isAdmin && building && (
            <div className="mt-3">
              <BuildAnApp workspaceId={workspaceId} scopes={directory.bot_scopes} />
            </div>
          )}
        </section>
      </section>

      {!isAdmin && <RequestApp workspaceId={workspaceId} />}
    </>
  );
}

function InstalledRow({ app, onOpen }: { app: InstalledApp; onOpen: () => void }) {
  const { t } = useTranslation('workspace');
  const Icon = KINDS[app.kind];
  return (
    <button
      onClick={onOpen}
      className="flex w-full items-center gap-3 rounded-lg border border-line/60 px-3 py-2 text-left hover:bg-raised/40 transition"
      data-qa="app-directory-row"
      data-kind={app.kind}
    >
      <Icon className="h-4 w-4 shrink-0 text-accent-soft" />
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-1.5">
          <span className="truncate text-sm font-medium text-fg-soft">{app.name}</span>
          <HealthBadge health={app.health} />
        </div>
        <div className="truncate text-xs text-muted">
          {t(`settings.apps.kinds.${app.kind}`)} · {displayNameOf(app.added_by_name)} · {channelText(app)}
        </div>
      </div>
      <ChevronRight className="h-4 w-4 shrink-0 text-muted" />
    </button>
  );
}
