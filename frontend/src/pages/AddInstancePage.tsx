import { useState, useEffect, type FormEvent } from 'react';
import { Button } from '@/shared/components/Button/Button';
import { TextInput } from '@/shared/components/Form/TextInput';
import { Link, useNavigate } from 'react-router';
import { useTranslation } from 'react-i18next';
import { KeyRound, ServerCrash, LogIn } from 'lucide-react';
import { useInstanceStore } from '@/stores/instances';
import { useInstanceInfo } from '@/features/instance/hooks/useInstanceInfo';
import { isTotpRequired } from '@/lib/errors';
import { PENDING_INVITE_KEY, ROUTES } from '@/shared/constants';
import { ssoStartUrl, startSsoInDesktop } from '@/features/instance/ssoStart';

/// A sign-in that started from an invite link owes the person the workspace
/// they were invited to, not the generic app shell.
function afterSignIn(): string {
  const pending = sessionStorage.getItem(PENDING_INVITE_KEY);
  if (!pending) return '/app';
  sessionStorage.removeItem(PENDING_INVITE_KEY);
  return `/invite/${pending}`;
}

export default function AddInstancePage() {
  const { t } = useTranslation('auth');
  const navigate = useNavigate();
  const { addInstance, instances, hydrated, loading, error, clearError } = useInstanceStore();

  const [url, setUrl] = useState(window.location.origin);
  const [wsUrl, setWsUrl] = useState('');
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [totpCode, setTotpCode] = useState('');
  const [needsTotp, setNeedsTotp] = useState(false);

  const { data: instance } = useInstanceInfo(window.location.origin);

  const sameOrigin = url.trim().replace(/\/$/, '') === window.location.origin;

  useEffect(() => {
    if (instance?.setup_required) navigate('/setup', { replace: true });
  }, [instance?.setup_required, navigate]);

  useEffect(() => {
    if (hydrated && instances.length > 0 && !url && !email && !password) {
      navigate(afterSignIn(), { replace: true });
    }
  }, [hydrated, instances.length, navigate, url, email, password]);

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    clearError();
    try {
      await addInstance(
        url.trim(),
        email.trim(),
        password,
        wsUrl.trim() || undefined,
        totpCode.trim() || undefined,
      );
      navigate(afterSignIn(), { replace: true });
    } catch (e) {
      // A password that was right but incomplete is not a failed sign-in, so the
      // form asks for the code instead of showing an error for something the
      // person did correctly.
      if (isTotpRequired(e)) setNeedsTotp(true);
    }
  };

  const isFirstInstance = instances.length === 0;

  return (
    <div className="min-h-screen bg-gradient-to-br from-app via-accent-deep to-app flex items-center justify-center p-4">
      <div className="w-full max-w-md">
        <div className="text-center mb-8">
          <div className="inline-flex items-center justify-center w-16 h-16 bg-purple-600 rounded-2xl mb-4">
            <ServerCrash className="w-8 h-8 text-fg" />
          </div>
          <h1 className="text-3xl font-bold text-fg">
            {isFirstInstance ? t('signIn.welcome') : t('signIn.addInstance')}
          </h1>
          <p className="text-muted mt-2">
            {isFirstInstance ? t('signIn.connectFirst') : t('signIn.connectAnother')}
          </p>
        </div>

        <form
          onSubmit={handleSubmit}
          className="bg-surface/50 backdrop-blur-xl border border-line rounded-2xl p-8 shadow-2xl"
        >
          {error && error !== 'totp_required' && (
            <div className="bg-red-500/10 border border-red-500/30 text-danger px-4 py-3 rounded-lg mb-6 text-sm">
              {error}
            </div>
          )}

          {needsTotp && (
            <div className="bg-purple-500/10 border border-purple-500/30 text-accent-soft px-4 py-3 rounded-lg mb-6 text-sm">
              {t('signIn.totpPrompt')}
            </div>
          )}

          <div className="space-y-5 mb-6">
            <TextInput
              label={t('signIn.serverUrl')}
              type="url"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              placeholder="https://chat.yourcompany.com"
              required
            />

            <div>
              <button
                type="button"
                onClick={() => setShowAdvanced(!showAdvanced)}
                className="text-xs text-muted hover:text-fg-dim transition flex items-center gap-1"
              >
                <span>{showAdvanced ? '▾' : '▸'}</span> {t('signIn.advancedOptions')}
              </button>
              {showAdvanced && (
                <TextInput
                  label={t('signIn.websocketUrl')}
                  optional
                  hint={t('signIn.websocketUrlHint')}
                  type="url"
                  value={wsUrl}
                  onChange={(e) => setWsUrl(e.target.value)}
                  placeholder="ws://localhost:3004"
                  className="mt-3"
                />
              )}
            </div>

            <TextInput
              label={t('signIn.email')}
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              placeholder="you@company.com"
              autoComplete="email"
              dataQa="login-email"
              required
            />

            <TextInput
              label={t('signIn.password')}
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder={t('signIn.passwordPlaceholder')}
              autoComplete="current-password"
              dataQa="login-password"
              required
            />

            {sameOrigin && (
              <div className="-mt-2 text-right">
                <Link
                  to={ROUTES.forgotPassword}
                  state={{ email: email.trim() }}
                  data-qa="forgot-password"
                  className="text-sm text-accent hover:text-accent-soft"
                >
                  {t('signIn.forgotPassword')}
                </Link>
              </div>
            )}

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
          </div>

          <Button
            variant="primary"
            fullWidth
            type="submit"
            disabled={loading}
            loading={loading}
            icon={<LogIn className="h-4 w-4" />}
            data-qa="login-submit"
          >
            {t('signIn.submit')}
          </Button>

          {instance?.sso_enabled && sameOrigin && (
            <>
              <div className="flex items-center gap-3 my-6">
                <div className="h-px flex-1 bg-raised" />
                <span className="text-xs uppercase tracking-wide text-subtle">{t('signIn.or')}</span>
                <div className="h-px flex-1 bg-raised" />
              </div>
              <a
                href={ssoStartUrl(sessionStorage.getItem(PENDING_INVITE_KEY))}
                onClick={(event) => {
                  startSsoInDesktop(event, sessionStorage.getItem(PENDING_INVITE_KEY));
                  sessionStorage.removeItem(PENDING_INVITE_KEY);
                }}
                data-qa="sso-start"
                className="w-full flex items-center justify-center gap-2 px-4 py-3 bg-raised hover:bg-elevated text-fg font-medium rounded-lg transition"
              >
                <KeyRound className="w-4 h-4" />
                {t('signIn.sso')}
              </a>
            </>
          )}
        </form>

        {!isFirstInstance && (
          <button
            onClick={() => navigate(-1)}
            className="mt-4 w-full text-center text-muted text-sm hover:text-fg-dim transition"
          >
            {t('signIn.cancel')}
          </button>
        )}

        <p className="text-center text-muted text-sm mt-6">{t('signIn.inviteOnly')}</p>
      </div>
    </div>
  );
}
