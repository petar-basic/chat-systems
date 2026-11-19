import { useState, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { AlertTriangle, Plug } from 'lucide-react';
import { Button } from '@/shared/components/Button/Button';
import { TextInput } from '@/shared/components/Form/TextInput';
import { Checkbox } from '@/shared/components/Form/Checkbox';
import { CopyField } from '@/shared/components/CopyField/CopyField';
import { toUserMessage } from '@/lib/errors';
import { formatDateTime } from '@/lib/datetime';
import { useBotAppActions, type AppEvent, type BotApp } from '@/features/workspace/hooks/useBotApps';

const EVENTS: AppEvent[] = [
  'message',
  'app_mention',
  'reaction_added',
  'reaction_removed',
  'member_joined_channel',
  'channel_created',
  'channel_archived',
  'app_home_opened',
  'link_shared',
  'function_executed',
];

interface Props {
  app: BotApp;
}

export function BotAppEvents({ app }: Props) {
  const { t } = useTranslation('workspace');
  const { configure, makeAppToken, revokeAppToken, revealSecret } = useBotAppActions(app.workspace_id);
  const [events, setEvents] = useState<AppEvent[]>(app.events);
  const [requestUrl, setRequestUrl] = useState(app.request_url ?? '');
  const [socketMode, setSocketMode] = useState(app.socket_mode);
  const [interactivity, setInteractivity] = useState(app.interactivity);
  const [home, setHome] = useState(app.home_enabled);
  const [appToken, setAppToken] = useState<string | null>(null);

  const toggle = (event: AppEvent, on: boolean) =>
    setEvents((current) => (on ? [...current, event] : current.filter((e) => e !== event)));

  const submit = (e: FormEvent) => {
    e.preventDefault();
    configure.mutate({
      appId: app.id,
      settings: {
        events,
        request_url: requestUrl.trim(),
        socket_mode: socketMode,
        interactivity,
        home_enabled: home,
      },
    });
  };

  const failure = configure.error ?? makeAppToken.error ?? revokeAppToken.error ?? revealSecret.error;

  return (
    <form onSubmit={submit} className="mt-3 space-y-3 border-t border-line/50 pt-3" data-qa="bot-app-events">
      {app.failing_since && (
        <p className="flex items-start gap-1.5 text-xs text-danger" data-qa="bot-app-failing">
          <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
          <span>
            {app.last_failure
              ? t('settings.apps.events.failingSinceReason', {
                  date: formatDateTime(app.failing_since),
                  reason: app.last_failure,
                })
              : t('settings.apps.events.failingSince', { date: formatDateTime(app.failing_since) })}
          </span>
        </p>
      )}
      <fieldset className="space-y-2">
        <legend className="mb-1 text-xs font-medium text-fg-dim">{t('settings.apps.events.legend')}</legend>
        {EVENTS.map((event) => (
          <Checkbox
            key={event}
            label={event}
            description={t(`settings.apps.events.kinds.${event}`)}
            checked={events.includes(event)}
            onChange={(on) => toggle(event, on)}
            dataQa={`bot-app-event-${event}`}
          />
        ))}
      </fieldset>
      <Checkbox
        label={t('settings.apps.events.socketMode')}
        description={t('settings.apps.events.socketModeHint')}
        checked={socketMode}
        onChange={setSocketMode}
        dataQa="bot-app-socket-mode"
      />
      <Checkbox
        label={t('settings.apps.events.interactivity')}
        description={t(
          socketMode ? 'settings.apps.events.interactivitySocket' : 'settings.apps.events.interactivityUrl',
        )}
        checked={interactivity}
        onChange={setInteractivity}
        dataQa="bot-app-interactivity"
      />
      <Checkbox
        label={t('settings.apps.events.home')}
        description={t('settings.apps.events.homeHint')}
        checked={home}
        onChange={setHome}
        dataQa="bot-app-home"
      />
      {!socketMode && (
        <TextInput
          label={t('settings.apps.events.requestUrl')}
          hint={t('settings.apps.events.requestUrlHint')}
          value={requestUrl}
          onChange={(e) => setRequestUrl(e.target.value)}
          placeholder="https://bot.example.com/events"
          dataQa="bot-app-request-url"
        />
      )}
      <div className="flex flex-wrap items-center gap-2">
        <Button
          type="submit"
          variant="primary"
          size="sm"
          loading={configure.isPending}
          data-qa="bot-app-events-save"
        >
          {t('settings.apps.events.save')}
        </Button>
        {!socketMode && (
          <Button
            type="button"
            size="sm"
            loading={revealSecret.isPending}
            onClick={() => revealSecret.mutate(app.id)}
            data-qa="bot-app-signing-secret-show"
          >
            {t('settings.apps.events.showSecret')}
          </Button>
        )}
      </div>
      {revealSecret.data && !socketMode && (
        <CopyField
          label={t('settings.apps.hook.signingSecret')}
          value={revealSecret.data.signing_secret}
          valueQa="bot-app-signing-secret"
        />
      )}
      {app.socket_mode && (
        <div className="space-y-2 rounded-lg bg-raised/40 p-3" data-qa="bot-app-socket">
          <p className="flex items-center gap-1.5 text-xs text-muted" data-qa="bot-app-socket-count">
            <Plug className="h-3.5 w-3.5" />
            {app.connected_sockets === 0
              ? t('settings.apps.events.noSocket')
              : t('settings.apps.events.sockets', { count: app.connected_sockets })}
          </p>
          <div className="flex flex-wrap gap-2">
            <Button
              type="button"
              size="sm"
              loading={makeAppToken.isPending}
              onClick={() =>
                makeAppToken.mutate(app.id, { onSuccess: (made) => setAppToken(made.app_token) })
              }
              data-qa="bot-app-app-token-make"
            >
              {app.has_app_token
                ? t('settings.apps.events.newAppToken')
                : t('settings.apps.events.makeAppToken')}
            </Button>
            {app.has_app_token && (
              <Button
                type="button"
                size="sm"
                variant="ghost"
                loading={revokeAppToken.isPending}
                onClick={() => revokeAppToken.mutate(app.id, { onSuccess: () => setAppToken(null) })}
                data-qa="bot-app-app-token-revoke"
              >
                {t('settings.apps.events.revokeAppToken')}
              </Button>
            )}
          </div>
          {appToken && (
            <div data-qa="bot-app-app-token">
              <p className="text-xs text-fg-soft">{t('settings.apps.events.appTokenNote')}</p>
              <CopyField
                label={t('settings.apps.events.appToken')}
                value={appToken}
                valueQa="bot-app-app-token-value"
              />
            </div>
          )}
        </div>
      )}
      {failure && (
        <p role="alert" className="text-sm text-danger">
          {toUserMessage(failure, t('settings.apps.changeFailed'))}
        </p>
      )}
    </form>
  );
}
