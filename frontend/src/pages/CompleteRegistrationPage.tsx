import { useState, useEffect, useMemo, type FormEvent } from 'react';
import { useParams, useNavigate, useSearchParams } from 'react-router';
import { useQueryClient } from '@tanstack/react-query';
import { UserPlus, MessageSquare, LogIn } from 'lucide-react';
import { useCompleteRegistration } from '../hooks/queries/useAuth';
import { useInstanceStore } from '@/stores/instances';
import { instanceManager } from '@/lib/instances';
import { QUERY_KEYS, PENDING_INVITE_KEY } from '@/shared/constants';

interface InviteInfo {
  email: string;
  workspace_name: string | null;
  workspace_id: string | null;
  already_registered: boolean;
}

export default function CompleteRegistrationPage() {
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
  const [verifyError, setVerifyError] = useState<string | null>(null);
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
      .api.get<InviteInfo>(`/auth/invites/${token}/verify`)
      .then(setInviteInfo)
      .catch(() => setVerifyError('This invite link is invalid or has expired.'));
  }, [token, originUrl]);

  const handleJoin = async () => {
    if (!token) return;
    setJoinError(null);
    setJoining(true);
    try {
      const res = await instanceManager
        .get(originUrl)
        .api.post<{ workspace_id: string }>(`/auth/invites/${token}/accept`);
      setActiveInstance(originUrl);
      await queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaces() });
      navigate(`/app/${res.workspace_id}`, { replace: true });
    } catch (e) {
      setJoinError(e instanceof Error ? e.message : 'Could not join this workspace.');
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

  if (verifyError) {
    return (
      <Shell>
        <div className="bg-surface/50 backdrop-blur-xl border border-line rounded-2xl p-8 shadow-2xl max-w-md text-center">
          <div className="text-danger text-lg font-medium mb-2">Invalid Invite</div>
          <p className="text-muted">{verifyError}</p>
          <button
            onClick={() => navigate('/add-instance')}
            className="mt-6 px-6 py-2 bg-purple-600 hover:bg-purple-500 text-white rounded-lg transition cursor-pointer"
          >
            Go to Login
          </button>
        </div>
      </Shell>
    );
  }

  if (!inviteInfo || !hydrated) {
    return (
      <Shell>
        <div className="w-8 h-8 border-2 border-purple-500/30 border-t-purple-500 rounded-full animate-spin" />
      </Shell>
    );
  }

  const workspaceLabel = inviteInfo.workspace_name || 'the workspace';

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
              <button
                onClick={handleJoin}
                disabled={joining}
                className="w-full flex items-center justify-center gap-2 px-4 py-3 bg-purple-600 hover:bg-purple-500 disabled:bg-purple-600/50 text-white font-medium rounded-lg transition cursor-pointer disabled:cursor-not-allowed"
              >
                {joining ? (
                  <div className="w-5 h-5 border-2 border-white/30 border-t-white rounded-full animate-spin" />
                ) : (
                  <>
                    <UserPlus className="w-4 h-4" />
                    Join {workspaceLabel}
                  </>
                )}
              </button>
            ) : signedInAs ? (
              <>
                <p className="text-muted text-sm mb-6">
                  You're signed in as <span className="text-fg font-medium">{signedInAs}</span>, but this
                  invite was sent to <span className="text-fg font-medium">{inviteInfo.email}</span>.
                </p>
                <button
                  onClick={handleSwitchAccount}
                  className="w-full flex items-center justify-center gap-2 px-4 py-3 bg-purple-600 hover:bg-purple-500 text-white font-medium rounded-lg transition cursor-pointer"
                >
                  <LogIn className="w-4 h-4" />
                  Switch account
                </button>
              </>
            ) : (
              <>
                <p className="text-muted text-sm mb-6">
                  This email already has an account here. Sign in to accept the invite.
                </p>
                <button
                  onClick={handleSignInToJoin}
                  className="w-full flex items-center justify-center gap-2 px-4 py-3 bg-purple-600 hover:bg-purple-500 text-white font-medium rounded-lg transition cursor-pointer"
                >
                  <LogIn className="w-4 h-4" />
                  Sign in to join
                </button>
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
          <h1 className="text-3xl font-bold text-fg">Complete Registration</h1>
          <p className="text-muted mt-2">
            You've been invited to <span className="text-accent font-medium">{workspaceLabel}</span>
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
                : 'Registration failed'}
            </div>
          )}

          <div className="mb-4">
            <label className="block text-sm font-medium text-fg-dim mb-2">Email</label>
            <input
              type="email"
              value={inviteInfo.email}
              disabled
              className="w-full px-4 py-3 bg-raised/30 border border-line-strong/50 rounded-lg text-muted cursor-not-allowed"
            />
          </div>

          <div className="mb-4">
            <label htmlFor="displayName" className="block text-sm font-medium text-fg-dim mb-2">
              Display Name
            </label>
            <input
              id="displayName"
              type="text"
              value={displayName}
              onChange={(e) => setDisplayName(e.target.value)}
              className="w-full px-4 py-3 bg-raised/50 border border-line-strong rounded-lg text-fg placeholder-muted focus:outline-none focus:ring-2 focus:ring-purple-500 focus:border-transparent transition"
              placeholder="John Doe"
              required
            />
          </div>

          <div className="mb-4">
            <label htmlFor="password" className="block text-sm font-medium text-fg-dim mb-2">
              Password
            </label>
            <input
              id="password"
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              className="w-full px-4 py-3 bg-raised/50 border border-line-strong rounded-lg text-fg placeholder-muted focus:outline-none focus:ring-2 focus:ring-purple-500 focus:border-transparent transition"
              placeholder="Min 8 characters"
              minLength={8}
              required
            />
          </div>

          <div className="mb-6">
            <label htmlFor="confirmPassword" className="block text-sm font-medium text-fg-dim mb-2">
              Confirm Password
            </label>
            <input
              id="confirmPassword"
              type="password"
              value={confirmPassword}
              onChange={(e) => setConfirmPassword(e.target.value)}
              className="w-full px-4 py-3 bg-raised/50 border border-line-strong rounded-lg text-fg placeholder-muted focus:outline-none focus:ring-2 focus:ring-purple-500 focus:border-transparent transition"
              placeholder="Confirm your password"
              minLength={8}
              required
            />
            {confirmPassword && password !== confirmPassword && (
              <p className="text-danger text-xs mt-1">Passwords do not match</p>
            )}
          </div>

          <button
            type="submit"
            disabled={completeRegistration.isPending || password !== confirmPassword || password.length < 8}
            className="w-full flex items-center justify-center gap-2 px-4 py-3 bg-purple-600 hover:bg-purple-500 disabled:bg-purple-600/50 text-white font-medium rounded-lg transition cursor-pointer disabled:cursor-not-allowed"
          >
            {completeRegistration.isPending ? (
              <div className="w-5 h-5 border-2 border-white/30 border-t-white rounded-full animate-spin" />
            ) : (
              <>
                <UserPlus className="w-4 h-4" />
                Create Account
              </>
            )}
          </button>
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
  return (
    <div className="text-center mb-8">
      <div className="inline-flex items-center justify-center w-16 h-16 bg-purple-600 rounded-2xl mb-4">
        <MessageSquare className="w-8 h-8 text-fg" />
      </div>
      <h1 className="text-3xl font-bold text-fg">Join {workspaceLabel}</h1>
      <p className="text-muted mt-2">
        Invitation for <span className="text-accent font-medium">{email}</span>
      </p>
    </div>
  );
}
