import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/shared/components/Button/Button';
import { AppIcon } from '@/shared/components/AppIcon/AppIcon';
import { Checkbox } from '@/shared/components/Form/Checkbox';
import { TextInput } from '@/shared/components/Form/TextInput';
import { QueryState } from '@/shared/components/QueryState/QueryState';
import { useAppActions } from '@/features/workspace';
import { formatDateTime } from '@/lib/datetime';
import { useWorkspaceStore } from '@/stores/workspace';
import { useCalendars, useUpdateCalendar, type CalendarState } from './hooks/useCalendars';

const AGENDA_AT = /^\d{2}:\d{2}$/;

export default function CalendarPanel() {
  const { t } = useTranslation('profile');
  const calendars = useCalendars();

  return (
    <div className="space-y-4" data-qa="calendars">
      <p className="text-sm text-muted">{t('calendar.intro')}</p>
      <QueryState
        isLoading={calendars.isLoading}
        isError={calendars.isError}
        onRetry={() => void calendars.refetch()}
      >
        {calendars.data?.data.map((calendar) => (
          <CalendarCard key={calendar.provider} calendar={calendar} />
        ))}
      </QueryState>
    </div>
  );
}

function CalendarCard({ calendar }: { calendar: CalendarState }) {
  const { t } = useTranslation('profile');
  const workspaceId = useWorkspaceStore((s) => s.currentWorkspace?.id);
  const actions = useAppActions(workspaceId);
  const update = useUpdateCalendar();
  const settings = calendar.settings;
  const save = (changes: Parameters<typeof update.mutate>[0]['changes']) =>
    update.mutate({ provider: calendar.provider, changes });
  const connect = () =>
    actions.connect.mutate({
      provider: calendar.provider,
      returnTo: `${window.location.pathname}?preferences=calendar`,
    });

  return (
    <section className="rounded-lg border border-line/60 p-3" data-qa={`calendar-${calendar.provider}`}>
      <div className="flex items-center gap-3">
        <AppIcon provider={calendar.provider} size="md" />
        <div className="min-w-0 flex-1">
          <h3 className="text-sm font-semibold text-fg">{calendar.name}</h3>
          <p className="truncate text-xs text-muted" data-qa="calendar-account">
            {calendar.connected_account ??
              (calendar.set_up ? t('calendar.notConnected') : t('calendar.notSetUp'))}
          </p>
        </div>
        {calendar.connected_account ? (
          <button
            onClick={() => actions.disconnect.mutate(calendar.provider)}
            disabled={actions.disconnect.isPending}
            className="shrink-0 text-xs text-danger hover:underline"
            data-qa="calendar-disconnect"
          >
            {t('calendar.disconnect')}
          </button>
        ) : (
          calendar.set_up && (
            <Button
              variant="primary"
              size="sm"
              onClick={connect}
              disabled={actions.connect.isPending}
              data-qa="calendar-connect"
            >
              {t('calendar.connect')}
            </Button>
          )
        )}
      </div>

      {settings && (
        <div className="mt-3 space-y-3 border-t border-line/50 pt-3">
          <Checkbox
            label={t('calendar.status.label')}
            description={t('calendar.status.description')}
            checked={settings.status}
            onChange={(status) => save({ status })}
            dataQa="calendar-status"
          />
          <Checkbox
            label={t('calendar.reminders.label')}
            description={t('calendar.reminders.description')}
            checked={settings.reminders}
            onChange={(reminders) => save({ reminders })}
            dataQa="calendar-reminders"
          />
          <Checkbox
            label={t('calendar.agenda.label')}
            description={t('calendar.agenda.description')}
            checked={settings.agenda}
            onChange={(agenda) => save({ agenda })}
            dataQa="calendar-agenda"
          />
          {settings.agenda && (
            <AgendaTime
              key={settings.agenda_at}
              value={settings.agenda_at}
              onSave={(agenda_at) => save({ agenda_at })}
            />
          )}
          <p
            className={`text-xs ${settings.last_error ? 'text-danger' : 'text-muted'}`}
            data-qa="calendar-sync"
          >
            {settings.last_error
              ? t('calendar.syncFailed', { error: settings.last_error })
              : settings.last_synced_at
                ? t('calendar.synced', { at: formatDateTime(settings.last_synced_at) })
                : t('calendar.syncing')}
          </p>
        </div>
      )}
    </section>
  );
}

function AgendaTime({ value, onSave }: { value: string; onSave: (value: string) => void }) {
  const { t } = useTranslation('profile');
  const [draft, setDraft] = useState(value);
  const commit = () => {
    if (AGENDA_AT.test(draft) && draft !== value) onSave(draft);
    else setDraft(value);
  };

  return (
    <TextInput
      label={t('calendar.agendaAt')}
      hint={t('calendar.agendaAtHint')}
      type="time"
      compact
      className="max-w-40"
      value={draft}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={commit}
      dataQa="calendar-agenda-at"
    />
  );
}
