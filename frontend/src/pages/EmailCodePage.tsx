import { useState, type FormEvent, type ReactNode } from 'react';
import { Link, useLocation, useNavigate } from 'react-router';
import { useTranslation } from 'react-i18next';
import { Fingerprint, LogIn, Mail } from 'lucide-react';
import { Button } from '@/shared/components/Button/Button';
import { TextInput } from '@/shared/components/Form/TextInput';
import { Trans } from '@/shared/components/Trans/Trans';
import { useInstanceStore } from '@/stores/instances';
import { useInstanceInfo } from '@/features/instance/hooks/useInstanceInfo';
import { acceptsPasskey, isPasskeyRequired, isTotpRequired, toUserMessage } from '@/lib/errors';
import { passkeysSupported } from '@/lib/webauthn';
import { desktopShell } from '@/lib/desktopShell';
import { afterSignIn, SECOND_FACTOR_ANSWERS } from '@/lib/signIn';
import { api } from '@/lib/api';
import { ROUTES } from '@/shared/constants';

const CARD = 'bg-surface/50 backdrop-blur-xl border border-line rounded-2xl p-8 shadow-2xl';

function emailFrom(state: unknown): string {
  if (state && typeof state === 'object' && 'email' in state && typeof state.email === 'string')
    return state.email;
  return '';
}

function Shell({ title, subtitle, children }: { title: string; subtitle?: ReactNode; children: ReactNode }) {
  return (
    <div className="min-h-screen bg-gradient-to-br from-app via-accent-deep to-app flex items-center justify-center p-4">
      <div className="w-full max-w-md">
        <div className="text-center mb-8">
          <div className="inline-flex items-center justify-center w-16 h-16 bg-purple-600 rounded-2xl mb-4">
            <Mail className="w-8 h-8 text-fg" />
          </div>
          <h1 className="text-3xl font-bold text-fg">{title}</h1>
          {subtitle && <p className="text-muted mt-2">{subtitle}</p>}
        </div>
        {children}
      </div>
    </div>
  );
}

function BackToSignIn() {
  const { t } = useTranslation('auth');
  return (
    <Link to={ROUTES.addInstance} className="text-accent hover:text-accent-soft text-sm">
      {t('backToSignIn')}
    </Link>
  );
}

function Alert({ children }: { children: ReactNode }) {
  return (
    <div
      role="alert"
      className="bg-red-500/10 border border-red-500/30 text-danger px-4 py-3 rounded-lg text-sm"
    >
      {children}
    </div>
  );
}

function Prompt({ children }: { children: ReactNode }) {
  return (
    <div className="bg-purple-500/10 border border-purple-500/30 text-accent-soft px-4 py-3 rounded-lg text-sm">
      {children}
    </div>
  );
}

export default function EmailCodePage() {
  const { t } = useTranslation('auth');
  const location = useLocation();
  const navigate = useNavigate();
  const { data: instance } = useInstanceInfo(window.location.origin);
  const { addInstanceWithEmailCode, addInstanceWithPasskey, loading, error, clearError } = useInstanceStore();

  const [email, setEmail] = useState(() => emailFrom(location.state));
  const [sent, setSent] = useState(false);
  const [asking, setAsking] = useState(false);
  const [askError, setAskError] = useState<string | null>(null);
  const [code, setCode] = useState('');
  const [totpCode, setTotpCode] = useState('');
  const [needsTotp, setNeedsTotp] = useState(false);
  const [needsPasskey, setNeedsPasskey] = useState(false);
  const [passkeyAccepted, setPasskeyAccepted] = useState(false);

  const inDesktop = desktopShell() !== null;
  const startPasskeyInDesktop = desktopShell()?.startPasskey;
  const offerPasskey = (passkeyAccepted || needsPasskey) && !inDesktop && passkeysSupported();

  const ask = async (e?: FormEvent) => {
    e?.preventDefault();
    if (!email.trim()) return;
    setAsking(true);
    setAskError(null);
    clearError();
    try {
      await api.typed((c) => c.POST('/auth/email-code', { body: { email: email.trim() } }), {
        anonymous: true,
      });
      setSent(true);
      setCode('');
    } catch (err) {
      setAskError(toUserMessage(err, t('emailCode.askFailed')));
    }
    setAsking(false);
  };

  const signIn = async (e: FormEvent) => {
    e.preventDefault();
    clearError();
    try {
      await addInstanceWithEmailCode(
        window.location.origin,
        email.trim(),
        code.trim(),
        totpCode.trim() || undefined,
      );
      navigate(afterSignIn(), { replace: true });
    } catch (err) {
      if (isTotpRequired(err)) setNeedsTotp(true);
      if (isPasskeyRequired(err)) setNeedsPasskey(true);
      setPasskeyAccepted(acceptsPasskey(err));
    }
  };

  const signInWithPasskey = async () => {
    clearError();
    try {
      await addInstanceWithPasskey(window.location.origin, { email: email.trim(), email_code: code.trim() });
      navigate(afterSignIn(), { replace: true });
    } catch {
      return;
    }
  };

  if (instance && !instance.email_code_sign_in) {
    return (
      <Shell title={t('emailCode.offTitle')} subtitle={t('emailCode.offBody')}>
        <div className="text-center">
          <BackToSignIn />
        </div>
      </Shell>
    );
  }

  if (!sent) {
    return (
      <Shell title={t('emailCode.title')} subtitle={t('emailCode.subtitle')}>
        <form onSubmit={(e) => void ask(e)} className={`${CARD} space-y-5`}>
          {askError && <Alert>{askError}</Alert>}
          <TextInput
            label={t('emailCode.email')}
            type="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            placeholder="you@company.com"
            autoComplete="email"
            dataQa="email-code-email"
            required
            autoFocus
          />
          <Button
            variant="primary"
            fullWidth
            type="submit"
            disabled={asking || !email.trim()}
            loading={asking}
            icon={<Mail className="h-4 w-4" />}
            data-qa="email-code-send"
          >
            {t('emailCode.send')}
          </Button>
          <div className="text-center">
            <BackToSignIn />
          </div>
        </form>
      </Shell>
    );
  }

  return (
    <Shell
      title={t('emailCode.sentTitle')}
      subtitle={
        <Trans
          t={t}
          i18nKey="emailCode.sentBody"
          values={{ email: email.trim() }}
          components={{ email: <span className="font-medium text-fg" /> }}
        />
      }
    >
      <form onSubmit={(e) => void signIn(e)} className={`${CARD} space-y-5`}>
        {error && !SECOND_FACTOR_ANSWERS.has(error) && <Alert>{error}</Alert>}
        {askError && <Alert>{askError}</Alert>}
        {needsPasskey && <Prompt>{t('signIn.passkeyPrompt')}</Prompt>}
        {needsTotp && <Prompt>{t('signIn.totpPrompt')}</Prompt>}

        <TextInput
          label={t('emailCode.code')}
          inputMode="numeric"
          autoComplete="one-time-code"
          value={code}
          onChange={(e) => setCode(e.target.value.replace(/\D/g, '').slice(0, 6))}
          placeholder="123456"
          dataQa="email-code-input"
          required
          autoFocus
        />

        {needsTotp && (
          <TextInput
            label={t('signIn.totpCode')}
            inputMode="text"
            autoComplete="one-time-code"
            autoFocus
            value={totpCode}
            onChange={(e) => setTotpCode(e.target.value.trim())}
            dataQa="login-totp"
            placeholder="123456"
            required
          />
        )}

        {offerPasskey && (
          <Button
            variant="secondary"
            fullWidth
            onClick={() => void signInWithPasskey()}
            disabled={loading}
            icon={<Fingerprint className="h-4 w-4" />}
            data-qa="email-code-passkey"
          >
            {t('signIn.passkeyInstead')}
          </Button>
        )}

        {inDesktop && startPasskeyInDesktop && (passkeyAccepted || needsPasskey) && (
          <Button
            variant="secondary"
            fullWidth
            onClick={() => startPasskeyInDesktop()}
            icon={<Fingerprint className="h-4 w-4" />}
            data-qa="email-code-passkey"
          >
            {t('signIn.passkey')}
          </Button>
        )}

        <Button
          variant="primary"
          fullWidth
          type="submit"
          disabled={loading || code.length !== 6}
          loading={loading}
          icon={<LogIn className="h-4 w-4" />}
          data-qa="email-code-submit"
        >
          {t('emailCode.submit')}
        </Button>

        <div className="flex items-center justify-between">
          <button
            type="button"
            onClick={() => void ask()}
            disabled={asking}
            className="text-sm text-accent hover:text-accent-soft disabled:opacity-60"
            data-qa="email-code-resend"
          >
            {t('emailCode.resend')}
          </button>
          <BackToSignIn />
        </div>
      </form>
    </Shell>
  );
}
