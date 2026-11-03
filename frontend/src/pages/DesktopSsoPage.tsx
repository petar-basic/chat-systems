import { useEffect, useRef, useState, type ReactNode } from 'react';
import { Link, useSearchParams } from 'react-router';
import { KeyRound } from 'lucide-react';
import { Spinner } from '@/shared/components/Spinner/Spinner';
import { buttonStyles } from '@/shared/components/Button/buttonStyles';
import { ROUTES } from '@/shared/constants';
import { instanceManager } from '@/lib/instances';
import { toUserMessage } from '@/lib/errors';
import { openUrl, replacePage } from '@/lib/pageLocation';

const TOKEN = /^[A-Za-z0-9_-]{16,128}$/;

interface Props {
  step: 'handoff' | 'complete';
}

export default function DesktopSsoPage({ step }: Props) {
  const [params] = useSearchParams();
  const code = params.get('code') ?? '';
  const nonce = params.get('nonce') ?? '';
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
