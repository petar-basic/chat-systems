import { useEffect, useRef, useState, type ReactNode } from 'react';
import { Link, useSearchParams } from 'react-router';
import { Fingerprint, KeyRound } from 'lucide-react';
import { Spinner } from '@/shared/components/Spinner/Spinner';
import { Button } from '@/shared/components/Button/Button';
import { buttonStyles } from '@/shared/components/Button/buttonStyles';
import { ROUTES } from '@/shared/constants';
import { instanceManager } from '@/lib/instances';
import { toUserMessage } from '@/lib/errors';
import { openUrl, replacePage } from '@/lib/pageLocation';
import { passkeysSupported, requestPasskey, wasCancelled } from '@/lib/webauthn';

const TOKEN = /^[A-Za-z0-9_-]{16,128}$/;

interface Props {
  step: 'handoff' | 'complete' | 'passkey';
}

export default function DesktopSsoPage({ step }: Props) {
  const [params] = useSearchParams();
  const code = params.get('code') ?? '';
  const nonce = params.get('nonce') ?? '';
  if (step === 'passkey') return <Passkey nonce={nonce} />;
  return step === 'handoff' ? <Handoff code={code} nonce={nonce} /> : <Complete code={code} />;
}

function Shell({ title, subtitle, children }: { title: string; subtitle: string; children?: ReactNode }) {
  return (
    <div className="min-h-screen bg-gradient-to-br from-app via-accent-deep to-app flex items-center justify-center p-4">
      <div className="w-full max-w-md text-center">
        <div className="inline-flex items-center justify-center w-16 h-16 bg-purple-600 rounded-2xl mb-4">
          <KeyRound className="w-8 h-8 text-fg" />
        </div>
        <h1 className="text-3xl font-bold text-fg">{title}</h1>
        <p className="text-muted mt-2">{subtitle}</p>
        {children && <div className="mt-8 flex flex-col items-center gap-4">{children}</div>}
      </div>
    </div>
  );
}

function Passkey({ nonce }: { nonce: string }) {
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  if (!TOKEN.test(nonce)) {
    return <Shell title="This link is broken" subtitle="Start signing in again from the Chat Systems app." />;
  }
  if (!passkeysSupported()) {
    return (
      <Shell
        title="No passkeys here"
        subtitle="This browser cannot use passkeys. Open the link in another one, or sign in with your password in the app."
      />
    );
  }

  const signIn = async () => {
    setBusy(true);
    setError(null);
    try {
      const api = instanceManager.get(window.location.origin).api;
      const challenge = await api.typed((c) => c.POST('/auth/passkeys/sign-in/start'), { anonymous: true });
      const credential = await requestPasskey(challenge.options);
      const { code } = await api.typed(
        (c) =>
          c.POST('/auth/passkeys/sign-in/desktop', {
            body: { challenge_id: challenge.challenge_id, credential },
          }),
        { anonymous: true },
      );
      replacePage(`/sso/desktop?code=${encodeURIComponent(code)}&nonce=${encodeURIComponent(nonce)}`);
    } catch (err) {
      if (!wasCancelled(err)) setError(toUserMessage(err, 'Could not sign in with a passkey. Try again.'));
      setBusy(false);
    }
  };

  return (
    <Shell
      title="Sign in with a passkey"
      subtitle="The Chat Systems app sent you here to use a passkey on this device. It takes you back to the app afterwards."
    >
      {error && (
        <p className="text-sm text-danger" role="alert">
          {error}
        </p>
      )}
      <Button
        variant="primary"
        onClick={() => void signIn()}
        loading={busy}
        icon={<Fingerprint className="h-4 w-4" />}
        data-qa="desktop-passkey"
      >
        Use a passkey
      </Button>
    </Shell>
  );
}

function Handoff({ code, nonce }: { code: string; nonce: string }) {
  const valid = TOKEN.test(code) && TOKEN.test(nonce);
  const appUrl = `chat-systems://sso?code=${code}&nonce=${nonce}`;

  useEffect(() => {
    if (valid) openUrl(appUrl);
  }, [valid, appUrl]);

  if (!valid) {
    return <Shell title="This link is broken" subtitle="Start signing in again from the Chat Systems app." />;
  }

  return (
    <Shell
      title="Back to the app"
      subtitle="You're signed in. Chat Systems opens on its own; if it doesn't, open it here. You can close this tab afterwards."
    >
      <a href={appUrl} className={buttonStyles({ variant: 'primary' })} data-qa="sso-open-app">
        Open Chat Systems
      </a>
    </Shell>
  );
}

function Complete({ code }: { code: string }) {
  const [error, setError] = useState<string | null>(TOKEN.test(code) ? null : 'This sign-in link is broken.');
  const started = useRef(false);

  useEffect(() => {
    if (started.current || !TOKEN.test(code)) return;
    started.current = true;
    instanceManager
      .get(window.location.origin)
      .api.typed((c) => c.POST('/auth/oidc/exchange', { body: { code } }), { anonymous: true })
      .then(({ landing }) => {
        const path = landing.startsWith('/') && !landing.startsWith('//') ? landing : ROUTES.app;
        replacePage(`${path}${path.includes('?') ? '&' : '?'}sso=1`);
      })
      .catch((err: unknown) => setError(toUserMessage(err, 'Could not finish signing in. Try again.')));
  }, [code]);

  if (error) {
    return (
      <Shell title="Sign-in didn't finish" subtitle={error}>
        <Link to={ROUTES.addInstance} className="text-accent hover:text-accent-soft text-sm">
          Back to sign in
        </Link>
      </Shell>
    );
  }

  return (
    <Shell title="Signing you in" subtitle="One moment.">
      <Spinner className="text-accent" />
    </Shell>
  );
}
