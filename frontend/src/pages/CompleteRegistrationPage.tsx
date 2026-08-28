import { useState, useEffect, type FormEvent } from 'react';
import { useParams, useNavigate, useSearchParams } from 'react-router';
import { useCompleteRegistration, useCurrentUser } from '../hooks/queries/useAuth';
import { api } from '../lib/api';
import { UserPlus, MessageSquare } from 'lucide-react';

interface InviteInfo {
  email: string;
  workspace_name: string | null;
  workspace_id: string | null;
}

export default function CompleteRegistrationPage() {
  const { token: pathToken } = useParams<{ token: string }>();
  const [searchParams] = useSearchParams();
  const token = pathToken || searchParams.get('token') || undefined;
  const navigate = useNavigate();
  const { data: user } = useCurrentUser();
  const completeRegistration = useCompleteRegistration();

  const [inviteInfo, setInviteInfo] = useState<InviteInfo | null>(null);
  const [verifyError, setVerifyError] = useState<string | null>(null);
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [displayName, setDisplayName] = useState('');

  useEffect(() => {
    if (user) {
      navigate('/app');
    }
  }, [user, navigate]);

  useEffect(() => {
    if (!token) return;
    api
      .get<InviteInfo>(`/auth/invites/${token}/verify`)
      .then(setInviteInfo)
      .catch(() => setVerifyError('This invite link is invalid or has expired.'));
  }, [token]);

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();

    if (password !== confirmPassword) {
      return;
    }
    if (password.length < 8) {
      return;
    }

    if (token) {
      completeRegistration.mutate({ token, password, displayName });
    }
  };

  if (verifyError) {
    return (
      <div className="min-h-screen bg-gradient-to-br from-app via-accent-deep to-app flex items-center justify-center p-4">
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
      </div>
    );
  }

  if (!inviteInfo) {
    return (
      <div className="min-h-screen bg-gradient-to-br from-app via-accent-deep to-app flex items-center justify-center">
        <div className="w-8 h-8 border-2 border-purple-500/30 border-t-purple-500 rounded-full animate-spin" />
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-gradient-to-br from-app via-accent-deep to-app flex items-center justify-center p-4">
      <div className="w-full max-w-md">
        <div className="text-center mb-8">
          <div className="inline-flex items-center justify-center w-16 h-16 bg-purple-600 rounded-2xl mb-4">
            <MessageSquare className="w-8 h-8 text-fg" />
          </div>
          <h1 className="text-3xl font-bold text-fg">Complete Registration</h1>
          <p className="text-muted mt-2">
            You've been invited to{' '}
            <span className="text-accent font-medium">{inviteInfo.workspace_name || 'a workspace'}</span>
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
    </div>
  );
}
