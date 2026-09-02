import { useState, useEffect, type FormEvent } from 'react';
import { useNavigate } from 'react-router';
import { useQuery } from '@tanstack/react-query';
import { KeyRound, ServerCrash, LogIn } from 'lucide-react';
import { useInstanceStore } from '../stores/instances';
import { instanceManager } from '../lib/instances';
import { isTotpRequired } from '@/lib/errors';
import { PENDING_INVITE_KEY } from '@/shared/constants';

/// A sign-in that started from an invite link owes the person the workspace
/// they were invited to, not the generic app shell.
function afterSignIn(): string {
  const pending = sessionStorage.getItem(PENDING_INVITE_KEY);
  if (!pending) return '/app';
  sessionStorage.removeItem(PENDING_INVITE_KEY);
  return `/invite/${pending}`;
}

export default function AddInstancePage() {
  const navigate = useNavigate();
  const { addInstance, instances, hydrated, loading, error, clearError } = useInstanceStore();

  const [url, setUrl] = useState(window.location.origin);
  const [wsUrl, setWsUrl] = useState('');
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [totpCode, setTotpCode] = useState('');
  const [needsTotp, setNeedsTotp] = useState(false);

  const { data: instance } = useQuery({
    queryKey: ['instance-info'],
    queryFn: () => instanceManager.get(window.location.origin).api.typed((c) => c.GET('/instance/info')),
    staleTime: Infinity,
  });

  const sameOrigin = url.trim().replace(/\/$/, '') === window.location.origin;

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
            {isFirstInstance ? 'Welcome to Chat Systems' : 'Add Instance'}
          </h1>
          <p className="text-muted mt-2">
            {isFirstInstance
              ? 'Connect to your Chat Systems server'
              : 'Connect to another Chat Systems server'}
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
              Enter the six-digit code from your authenticator app, or one of your recovery codes.
            </div>
          )}

          <div className="mb-5">
            <label htmlFor="url" className="block text-sm font-medium text-fg-dim mb-2">
              Server URL
            </label>
            <input
              id="url"
              type="url"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              className="w-full px-4 py-3 bg-raised/50 border border-line-strong rounded-lg text-fg placeholder-muted focus:outline-none focus:ring-2 focus:ring-purple-500 focus:border-transparent transition"
              placeholder="https://chat.yourcompany.com"
              required
            />
          </div>

          <div className="mb-5">
            <button
              type="button"
              onClick={() => setShowAdvanced(!showAdvanced)}
              className="text-xs text-muted hover:text-fg-dim transition flex items-center gap-1"
            >
              <span>{showAdvanced ? '▾' : '▸'}</span> Advanced options
            </button>
            {showAdvanced && (
              <div className="mt-3">
                <label htmlFor="wsUrl" className="block text-sm font-medium text-fg-dim mb-2">
                  WebSocket URL
                  <span className="ml-2 text-xs text-muted font-normal">
                    (optional — only needed if WS runs on a different port)
                  </span>
                </label>
                <input
                  id="wsUrl"
                  type="url"
                  value={wsUrl}
                  onChange={(e) => setWsUrl(e.target.value)}
                  className="w-full px-4 py-3 bg-raised/50 border border-line-strong rounded-lg text-fg placeholder-muted focus:outline-none focus:ring-2 focus:ring-purple-500 focus:border-transparent transition"
                  placeholder="ws://localhost:3004"
                />
              </div>
            )}
          </div>

          <div className="mb-5">
            <label htmlFor="email" className="block text-sm font-medium text-fg-dim mb-2">
              Email
            </label>
            <input
              id="email"
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              className="w-full px-4 py-3 bg-raised/50 border border-line-strong rounded-lg text-fg placeholder-muted focus:outline-none focus:ring-2 focus:ring-purple-500 focus:border-transparent transition"
              placeholder="you@company.com"
              required
            />
          </div>

          <div className="mb-6">
            <label htmlFor="password" className="block text-sm font-medium text-fg-dim mb-2">
              Password
            </label>
            <input
              id="password"
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              className="w-full px-4 py-3 bg-raised/50 border border-line-strong rounded-lg text-fg placeholder-muted focus:outline-none focus:ring-2 focus:ring-purple-500 focus:border-transparent transition"
              placeholder="Enter your password"
              required
            />
          </div>

          {needsTotp && (
            <div className="mb-6">
              <label htmlFor="totp" className="block text-sm font-medium text-fg-dim mb-2">
                Authentication code
              </label>
              <input
                id="totp"
                type="text"
                inputMode="text"
                autoComplete="one-time-code"
                autoFocus
                value={totpCode}
                onChange={(e) => setTotpCode(e.target.value.trim())}
                data-qa="login-totp"
                className="w-full px-4 py-3 bg-raised/50 border border-line-strong rounded-lg text-fg placeholder-muted focus:outline-none focus:ring-2 focus:ring-purple-500 focus:border-transparent transition tracking-widest"
                placeholder="123456"
                required
              />
            </div>
          )}

          <button
            type="submit"
            disabled={loading}
            className="w-full flex items-center justify-center gap-2 px-4 py-3 bg-purple-600 hover:bg-purple-500 disabled:bg-purple-600/50 text-white font-medium rounded-lg transition cursor-pointer disabled:cursor-not-allowed"
          >
            {loading ? (
              <div className="w-5 h-5 border-2 border-white/30 border-t-white rounded-full animate-spin" />
            ) : (
              <>
                <LogIn className="w-4 h-4" />
                Connect
              </>
            )}
          </button>

          {instance?.sso_enabled && sameOrigin && (
            <>
              <div className="flex items-center gap-3 my-6">
                <div className="h-px flex-1 bg-raised" />
                <span className="text-xs uppercase tracking-wide text-subtle">or</span>
                <div className="h-px flex-1 bg-raised" />
              </div>
              <a
                href={`${window.location.origin}/api/auth/oidc/start`}
                data-qa="sso-start"
                className="w-full flex items-center justify-center gap-2 px-4 py-3 bg-raised hover:bg-elevated text-fg font-medium rounded-lg transition"
              >
                <KeyRound className="w-4 h-4" />
                Sign in with SSO
              </a>
            </>
          )}
        </form>

        {!isFirstInstance && (
          <button
            onClick={() => navigate(-1)}
            className="mt-4 w-full text-center text-muted text-sm hover:text-fg-dim transition"
          >
            Cancel
          </button>
        )}

        <p className="text-center text-muted text-sm mt-6">
          Invite-only platform. Contact your admin for access.
        </p>
      </div>
    </div>
  );
}
