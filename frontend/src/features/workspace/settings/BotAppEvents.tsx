import { useState, type FormEvent } from 'react';
import { AlertTriangle, Plug } from 'lucide-react';
import { Button } from '@/shared/components/Button/Button';
import { TextInput } from '@/shared/components/Form/TextInput';
import { Checkbox } from '@/shared/components/Form/Checkbox';
import { CopyField } from '@/shared/components/CopyField/CopyField';
import { toUserMessage } from '@/lib/errors';
import { formatDateTime } from '@/lib/datetime';
import { useBotAppActions, type AppEvent, type BotApp } from '@/features/workspace/hooks/useBotApps';

const EVENTS: { event: AppEvent; label: string }[] = [
  { event: 'message', label: 'A message in a channel it is in' },
  { event: 'app_mention', label: 'Somebody mentions it' },
  { event: 'reaction_added', label: 'A reaction is added in a channel it is in' },
  { event: 'reaction_removed', label: 'A reaction is removed in a channel it is in' },
  { event: 'member_joined_channel', label: 'Somebody joins a channel it is in' },
  { event: 'channel_created', label: 'A public channel is made' },
  { event: 'channel_archived', label: 'A channel it is in is archived' },
];

interface Props {
  app: BotApp;
}

export function BotAppEvents({ app }: Props) {
  const { configure, makeAppToken, revokeAppToken, revealSecret } = useBotAppActions(app.workspace_id);
  const [events, setEvents] = useState<AppEvent[]>(app.events);
  const [requestUrl, setRequestUrl] = useState(app.request_url ?? '');
  const [socketMode, setSocketMode] = useState(app.socket_mode);
  const [appToken, setAppToken] = useState<string | null>(null);

  const toggle = (event: AppEvent, on: boolean) =>
    setEvents((current) => (on ? [...current, event] : current.filter((e) => e !== event)));

  const submit = (e: FormEvent) => {
    e.preventDefault();
    configure.mutate({
      appId: app.id,
      settings: { events, request_url: requestUrl.trim(), socket_mode: socketMode },
    });
  };

  const failure = configure.error ?? makeAppToken.error ?? revokeAppToken.error ?? revealSecret.error;

  return (
    <form onSubmit={submit} className="mt-3 space-y-3 border-t border-line/50 pt-3" data-qa="bot-app-events">
      {app.failing_since && (
        <p className="flex items-start gap-1.5 text-xs text-danger" data-qa="bot-app-failing">
          <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
          <span>
            Failing since {formatDateTime(app.failing_since)}
            {app.last_failure ? `: ${app.last_failure}` : ''}
          </span>
        </p>
      )}
      <fieldset className="space-y-2">
        <legend className="mb-1 text-xs font-medium text-fg-dim">Events</legend>
        {EVENTS.map(({ event, label }) => (
          <Checkbox
            key={event}
            label={event}
            description={label}
            checked={events.includes(event)}
            onChange={(on) => toggle(event, on)}
            dataQa={`bot-app-event-${event}`}
          />
        ))}
      </fieldset>
      <Checkbox
        label="Socket mode"
        description="The app opens a socket to this instance and takes its events and slash commands there, so it needs no public URL."
        checked={socketMode}
        onChange={setSocketMode}
        dataQa="bot-app-socket-mode"
      />
      {!socketMode && (
        <TextInput
          label="Request URL"
          hint="Events are posted here, signed with the signing secret. The URL has to answer a challenge before it is saved."
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
          Save events
        </Button>
        {!socketMode && (
          <Button
            type="button"
            size="sm"
            loading={revealSecret.isPending}
            onClick={() => revealSecret.mutate(app.id)}
            data-qa="bot-app-signing-secret-show"
          >
            Show signing secret
          </Button>
        )}
      </div>
      {revealSecret.data && !socketMode && (
        <CopyField
          label="Signing secret"
          value={revealSecret.data.signing_secret}
          valueQa="bot-app-signing-secret"
        />
      )}
      {app.socket_mode && (
        <div className="space-y-2 rounded-lg bg-raised/40 p-3" data-qa="bot-app-socket">
          <p className="flex items-center gap-1.5 text-xs text-muted" data-qa="bot-app-socket-count">
            <Plug className="h-3.5 w-3.5" />
            {app.connected_sockets === 0
              ? 'No socket is open'
              : `${app.connected_sockets} ${app.connected_sockets === 1 ? 'socket' : 'sockets'} open`}
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
              {app.has_app_token ? 'New app-level token' : 'Make an app-level token'}
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
                Revoke it
              </Button>
            )}
          </div>
          {appToken && (
            <div data-qa="bot-app-app-token">
              <p className="text-xs text-fg-soft">
                Copy it now; it is not shown again. It opens sockets and does nothing else, and a new one
                closes every socket the old one opened.
              </p>
              <CopyField label="App-level token" value={appToken} valueQa="bot-app-app-token-value" />
            </div>
          )}
        </div>
      )}
      {failure && (
        <p role="alert" className="text-sm text-danger">
          {toUserMessage(failure, 'The change failed')}
        </p>
      )}
    </form>
  );
}
