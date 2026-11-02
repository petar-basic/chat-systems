import { useState, type FormEvent, type ReactNode } from 'react';
import { KeyRound } from 'lucide-react';
import { buttonStyles } from '@/shared/components/Button/buttonStyles';
import { Button } from '@/shared/components/Button/Button';
import { TextInput } from '@/shared/components/Form/TextInput';
import { useSearchParams, useLocation, Link } from 'react-router';
import { Trans, useTranslation } from 'react-i18next';
import { ROUTES } from '@/shared/constants';
import { api } from '../lib/api';

export default function ResetPasswordPage() {
  const [searchParams] = useSearchParams();
  const token = searchParams.get('token');

  if (!token) {
    return <ForgotPasswordForm />;
  }

  return <ResetForm token={token} />;
}

function emailFrom(state: unknown): string {
  if (state && typeof state === 'object' && 'email' in state && typeof state.email === 'string')
    return state.email;
  return '';
}

const CARD = 'bg-surface/50 backdrop-blur-xl border border-line rounded-2xl p-8 shadow-2xl';

function Shell({ title, subtitle, children }: { title: string; subtitle?: string; children: ReactNode }) {
  return (
    <div className="min-h-screen bg-gradient-to-br from-app via-accent-deep to-app flex items-center justify-center p-4">
      <div className="w-full max-w-md">
        <div className="text-center mb-8">
          <div className="inline-flex items-center justify-center w-16 h-16 bg-purple-600 rounded-2xl mb-4">
            <KeyRound className="w-8 h-8 text-fg" />
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

function ForgotPasswordForm() {
  const { t } = useTranslation('auth');
  const location = useLocation();
  const [email, setEmail] = useState(() => emailFrom(location.state));
  const [submitted, setSubmitted] = useState(false);
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    if (!email.trim()) return;
    setLoading(true);
    await api
      .typed((c) => c.POST('/auth/forgot-password', { body: { email: email.trim() } }), { anonymous: true })
      .catch(() => undefined);
    setSubmitted(true);
    setLoading(false);
  };

  if (submitted) {
    return (
      <Shell title={t('forgotPassword.sentTitle')}>
        <div className={`${CARD} text-center space-y-4`} data-qa="forgot-password-sent">
          <p className="text-fg-dim">
            <Trans
              t={t}
              i18nKey="forgotPassword.sentBody"
              values={{ email: email.trim() }}
              components={{ email: <span className="font-medium text-fg" /> }}
            />
          </p>
          <BackToSignIn />
        </div>
      </Shell>
    );
  }

  return (
    <Shell title={t('forgotPassword.title')} subtitle={t('forgotPassword.subtitle')}>
      <form onSubmit={handleSubmit} className={`${CARD} space-y-5`}>
        <TextInput
          label={t('forgotPassword.email')}
          type="email"
          value={email}
          onChange={(e) => setEmail(e.target.value)}
          placeholder="you@company.com"
          autoComplete="email"
          dataQa="forgot-password-email"
          required
          autoFocus
        />
        <Button
          variant="primary"
          fullWidth
          type="submit"
          disabled={loading || !email.trim()}
          loading={loading}
        >
          {t('forgotPassword.submit')}
        </Button>
        <div className="text-center">
          <BackToSignIn />
        </div>
      </form>
    </Shell>
  );
}

function ResetForm({ token }: { token: string }) {
  const { t } = useTranslation('auth');
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState(false);
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    if (password.length < 8) {
      setError(t('resetPassword.tooShort'));
      return;
    }
    if (password !== confirm) {
      setError(t('resetPassword.mismatch'));
      return;
    }
    setLoading(true);
    setError(null);
    try {
      await api.typed((c) => c.POST('/auth/reset-password', { body: { token, password } }), {
        anonymous: true,
      });
      setSuccess(true);
    } catch (err) {
      setError(err instanceof Error ? err.message : t('resetPassword.failed'));
    }
    setLoading(false);
  };

  if (success) {
    return (
      <Shell title={t('resetPassword.doneTitle')}>
        <div className={`${CARD} text-center space-y-5`}>
          <p className="text-fg-dim">{t('resetPassword.doneBody')}</p>
          <Link to={ROUTES.addInstance} className={buttonStyles({ variant: 'primary' })}>
            {t('resetPassword.signIn')}
          </Link>
        </div>
      </Shell>
    );
  }

  return (
    <Shell title={t('resetPassword.title')}>
      <form onSubmit={handleSubmit} className={`${CARD} space-y-5`}>
        {error && (
          <div
            role="alert"
            className="bg-red-500/10 border border-red-500/30 text-danger px-4 py-3 rounded-lg text-sm"
          >
            {error}
          </div>
        )}
        <TextInput
          label={t('resetPassword.newPassword')}
          type="password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          hint={t('resetPassword.newPasswordHint')}
          autoComplete="new-password"
          dataQa="reset-password-new"
          required
          minLength={8}
          autoFocus
        />
        <TextInput
          label={t('resetPassword.confirmPassword')}
          type="password"
          value={confirm}
          onChange={(e) => setConfirm(e.target.value)}
          autoComplete="new-password"
          dataQa="reset-password-confirm"
          required
          minLength={8}
        />
        <Button variant="primary" fullWidth type="submit" disabled={loading} loading={loading}>
          {t('resetPassword.submit')}
        </Button>
      </form>
    </Shell>
  );
}
