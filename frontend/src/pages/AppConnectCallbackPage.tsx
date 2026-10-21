import { useEffect, useRef, useState } from 'react';
import { Link, useNavigate, useSearchParams } from 'react-router';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { takePendingConnect } from '@/lib/appConnect';
import { toUserMessage } from '@/lib/errors';

export default function AppConnectCallbackPage() {
  const [params] = useSearchParams();
  const navigate = useNavigate();
  const code = params.get('code');
  const state = params.get('state');
  const denied = params.get('error');
  const [error, setError] = useState<string | null>(() => {
    if (denied === 'access_denied') return 'You cancelled the connection.';
    if (denied || !code || !state) return 'The app did not send us back a code.';
    return null;
  });
  const started = useRef(false);

  useEffect(() => {
    if (started.current) return;
    started.current = true;
    const pending = takePendingConnect();
    if (!code || !state || denied) return;
    const api = getApiForInstance(pending?.instanceUrl);
    void (async () => {
      try {
        const done = await api.typed((c) => c.POST('/apps/connections', { body: { code, state } }));
        if (pending?.refreshMessageId) {
          await api
            .typed((c) =>
              c.POST('/messages/{msg_id}/unfurls/refresh', {
                params: { path: { msg_id: pending.refreshMessageId ?? '' } },
              }),
            )
            .catch(() => undefined);
        }
        navigate(done.return_to ?? `/app/${done.workspace_id}`, { replace: true });
      } catch (err) {
        setError(toUserMessage(err, "Couldn't connect the app."));
      }
    })();
  }, [code, state, denied, navigate]);

  return (
    <div className="h-dvh flex items-center justify-center bg-app text-fg p-6" data-qa="app-connect-callback">
      {error ? (
        <div className="max-w-sm text-center space-y-3">
          <p className="text-fg-soft">{error}</p>
          <Link to="/app" className="text-accent hover:underline">
            Back to the workspace
          </Link>
        </div>
      ) : (
        <p className="text-muted">Connecting…</p>
      )}
    </div>
  );
}
