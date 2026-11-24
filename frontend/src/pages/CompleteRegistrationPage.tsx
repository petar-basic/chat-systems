import { useState, useEffect, useMemo, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { Spinner } from '@/shared/components/Spinner/Spinner';
import { Button } from '@/shared/components/Button/Button';
import { TextInput } from '@/shared/components/Form/TextInput';
import { useParams, useNavigate, useSearchParams } from 'react-router';
import { useQueryClient } from '@tanstack/react-query';
import { UserPlus, MessageSquare, LogIn } from 'lucide-react';
import { useCompleteRegistration } from '@/features/instance/hooks/useAuth';
import { useInstanceStore } from '@/stores/instances';
import { instanceManager } from '@/lib/instances';
import { QUERY_KEYS, PENDING_INVITE_KEY } from '@/shared/constants';
import { SsoInviteButton } from '@/features/instance/SsoInviteButton';
import type { components } from '@/api/schema';
import { Trans } from '@/shared/components/Trans/Trans';

type InviteInfo = components['schemas']['InvitePreview'];

export default function CompleteRegistrationPage() {
  const { t } = useTranslation('auth');
  const { token: pathToken } = useParams<{ token: string }>();
  const [searchParams] = useSearchParams();
  const token = pathToken || searchParams.get('token') || undefined;
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const completeRegistration = useCompleteRegistration();

  const instances = useInstanceStore((s) => s.instances);
  const hydrated = useInstanceStore((s) => s.hydrated);
  const removeInstance = useInstanceStore((s) => s.removeInstance);
  const setActiveInstance = useInstanceStore((s) => s.setActiveInstance);

  const originUrl = instanceManager.normalize(window.location.origin);
  const localInstance = useMemo(
    () => instances.find((i) => i.url === originUrl) ?? null,
    [instances, originUrl],
  );

  const [inviteInfo, setInviteInfo] = useState<InviteInfo | null>(null);
  const [verifyFailed, setVerifyFailed] = useState(false);
  const [joinError, setJoinError] = useState<string | null>(null);
  const [joining, setJoining] = useState(false);
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [displayName, setDisplayName] = useState('');

  useEffect(() => {
    if (!token && hydrated && instances.length > 0) {
      navigate('/app');
    }
  }, [token, hydrated, instances.length, navigate]);

  useEffect(() => {
    if (!token) return;
    instanceManager
      .get(originUrl)
      .api.typed((c) => c.GET('/auth/invites/{token}/verify', { params: { path: { token } } }), {
        anonymous: true,
      })
      .then(setInviteInfo)
      .catch(() => setVerifyFailed(true));
  }, [token, originUrl]);

  const handleJoin = async () => {
    if (!token) return;
    setJoinError(null);
    setJoining(true);
    try {
      const res = await instanceManager
        .get(originUrl)
        .api.typed((c) => c.POST('/auth/invites/{token}/accept', { params: { path: { token } } }));
      setActiveInstance(originUrl);
      await queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaces() });
      navigate(`/app/${res.workspace_id}`, { replace: true });
    } catch (e) {
      setJoinError(e instanceof Error ? e.message : t('completeRegistration.joinFailed'));
      setJoining(false);
    }
  };

  const handleSignInToJoin = () => {
    if (token) sessionStorage.setItem(PENDING_INVITE_KEY, token);
    navigate('/add-instance');
  };

  const handleSwitchAccount = () => {
    removeInstance(originUrl);
    if (token) sessionStorage.setItem(PENDING_INVITE_KEY, token);
    navigate('/add-instance');
  };

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();

    if (password !== confirmPassword) {
      return;
    }
    if (password.length < 8) {
      return;
    }
    if (!token) return;

    try {
      await completeRegistration.mutateAsync({ token, password, displayName });
    } catch {
      return;
    }
    await queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaces() });
    navigate(inviteInfo?.workspace_id ? `/app/${inviteInfo.workspace_id}` : '/app', {
      replace: true,
    });
  };

  if (verifyFailed) {
    return (
      <Shell>
        <div className="bg-surface/50 backdrop-blur-xl border border-line rounded-2xl p-8 shadow-2xl max-w-md text-center">
          <div className="text-danger text-lg font-medium mb-2">{t('completeRegistration.invalidTitle')}</div>
          <p className="text-muted">{t('completeRegistration.invalidBody')}</p>
          <Button variant="primary" className="mt-6" onClick={() => navigate('/add-instance')}>
            {t('completeRegistration.goToSignIn')}
          </Button>
        </div>
      </Shell>
    );
  }

  if (!inviteInfo || !hydrated) {
    return (
      <Shell>
        <Spinner size="lg" className="text-accent" />
      </Shell>
    );
  }

  const workspaceLabel = inviteInfo.workspace_name || t('completeRegistration.workspaceFallback');

  if (inviteInfo.already_registered) {
    const signedInAs = localInstance?.user.email ?? null;
    const matches = signedInAs?.toLowerCase() === inviteInfo.email.toLowerCase();

    return (
      <Shell>
        <div className="w-full max-w-md">
          <InviteHeader workspaceLabel={workspaceLabel} email={inviteInfo.email} />

          <div className="bg-surface/50 backdrop-blur-xl border border-line rounded-2xl p-8 shadow-2xl text-center">
            {joinError && (
              <div className="bg-red-500/10 border border-red-500/30 text-danger px-4 py-3 rounded-lg mb-6 text-sm">
                {joinError}
              </div>
            )}

            {matches ? (
              <Button
                variant="primary"
                fullWidth
                onClick={handleJoin}
                disabled={joining}
                loading={joining}
                icon={<UserPlus className="h-4 w-4" />}
              >
                {t('completeRegistration.join', { name: workspaceLabel })}
              </Button>
            ) : signedInAs ? (
              <>
                <p className="text-muted text-sm mb-6">
                  <Trans
                    t={t}
                    i18nKey="completeRegistration.wrongAccount"
                    values={{ signedInAs, email: inviteInfo.email }}
                    components={{
                      account: <span className="text-fg font-medium" />,
                      email: <span className="text-fg font-medium" />,
                    }}
                  />
                </p>
                <Button variant="primary" fullWidth onClick={handleSwitchAccount}>
                  <LogIn className="w-4 h-4" />
                  {t('completeRegistration.switchAccount')}
                </Button>
              </>
            ) : (
              <>
                <p className="text-muted text-sm mb-6">{t('completeRegistration.hasAccount')}</p>
                <Button variant="primary" fullWidth onClick={handleSignInToJoin}>
                  <LogIn className="w-4 h-4" />
                  {t('completeRegistration.signInToJoin')}
                </Button>
                {token && <SsoInviteButton token={token} />}
              </>
            )}
          </div>
        </div>
      </Shell>
    );
  }

  return (
    <Shell>
      <div className="w-full max-w-md">
        <div className="text-center mb-8">
          <div className="inline-flex items-center justify-center w-16 h-16 bg-purple-600 rounded-2xl mb-4">
            <MessageSquare className="w-8 h-8 text-fg" />
          </div>
          <h1 className="text-3xl font-bold text-fg">{t('completeRegistration.title')}</h1>
          <p className="text-muted mt-2">
            <Trans
              t={t}
              i18nKey="completeRegistration.invitedTo"
              values={{ name: workspaceLabel }}
              components={{ workspace: <span className="text-accent font-medium" /> }}
            />
          </p>
        </div>

        <form
          onSubmit={handleSubmit}
          className="bg-surface/50 backdrop-blur-xl border border-line rounded-2xl p-8 shadow-2xl"
        >
          {completeRegistration.error && (
            <div className="bg-red-500/10 border border-red-500/30 text-danger px-4 py-3 rounded-lg mb-6 text-sm">
              {completeRegistration.error instanceof Error
                ? completeRegistration.error.message
                : t('completeRegistration.failed')}
            </div>
          )}

          <div className="space-y-4 mb-6">
            <TextInput label={t('signIn.email')} type="email" value={inviteInfo.email} disabled readOnly />
            <TextInput
              label={t('completeRegistration.displayName')}
              value={displayName}
              onChange={(e) => setDisplayName(e.target.value)}
              placeholder={t('completeRegistration.displayNamePlaceholder')}
              autoComplete="name"
              required
            />
            <TextInput
              label={t('signIn.password')}
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder={t('completeRegistration.passwordPlaceholder')}
              autoComplete="new-password"
              minLength={8}
              required
            />
            <TextInput
              label={t('completeRegistration.confirmPassword')}
              type="password"
              value={confirmPassword}
              onChange={(e) => setConfirmPassword(e.target.value)}
              placeholder={t('completeRegistration.confirmPlaceholder')}
              autoComplete="new-password"
              minLength={8}
              error={confirmPassword && password !== confirmPassword ? t('resetPassword.mismatch') : null}
              required
            />
          </div>

          <Button
            variant="primary"
            fullWidth
            type="submit"
            disabled={completeRegistration.isPending || password !== confirmPassword || password.length < 8}
            loading={completeRegistration.isPending}
            icon={<UserPlus className="h-4 w-4" />}
          >
            {t('completeRegistration.submit')}
          </Button>
          {token && <SsoInviteButton token={token} />}
        </form>
      </div>
    </Shell>
  );
}

function Shell({ children }: { children: React.ReactNode }) {
  return (
    <div className="min-h-screen bg-gradient-to-br from-app via-accent-deep to-app flex items-center justify-center p-4">
      {children}
    </div>
  );
}

function InviteHeader({ workspaceLabel, email }: { workspaceLabel: string; email: string }) {
  const { t } = useTranslation('auth');
  return (
    <div className="text-center mb-8">
      <div className="inline-flex items-center justify-center w-16 h-16 bg-purple-600 rounded-2xl mb-4">
        <MessageSquare className="w-8 h-8 text-fg" />
      </div>
      <h1 className="text-3xl font-bold text-fg">
        {t('completeRegistration.join', { name: workspaceLabel })}
      </h1>
      <p className="text-muted mt-2">
        <Trans
          t={t}
          i18nKey="completeRegistration.invitationFor"
          values={{ email }}
          components={{ email: <span className="text-accent font-medium" /> }}
        />
      </p>
    </div>
  );
}
