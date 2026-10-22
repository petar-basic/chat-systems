import { useState, type FormEvent } from 'react';
import { buttonStyles } from '@/shared/components/Button/buttonStyles';
import { Button } from '@/shared/components/Button/Button';
import { useSearchParams, Link } from 'react-router';
import { api } from '../lib/api';

export default function ResetPasswordPage() {
  const [searchParams] = useSearchParams();
  const token = searchParams.get('token');

  if (!token) {
    return <ForgotPasswordForm />;
  }

  return <ResetForm token={token} />;
}

function ForgotPasswordForm() {
  const [email, setEmail] = useState('');
  const [submitted, setSubmitted] = useState(false);
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    if (!email.trim()) return;
    setLoading(true);
    await api
      .typed((c) => c.POST('/auth/forgot-password', { body: { email: email.trim() } }))
      .catch(() => undefined);
    setSubmitted(true);
    setLoading(false);
  };

  return (
    <div className="min-h-screen flex items-center justify-center bg-app px-4">
      <div className="w-full max-w-sm">
        <div className="text-center mb-8">
          <div className="w-12 h-12 rounded-2xl bg-purple-600 flex items-center justify-center text-2xl font-bold text-white mx-auto mb-4">
            C
          </div>
          <h1 className="text-2xl font-bold text-fg">Reset password</h1>
          <p className="text-muted mt-2">Enter your email to receive a reset link</p>
        </div>

        {submitted ? (
          <div className="bg-surface border border-line rounded-2xl p-6 text-center">
            <p className="text-success mb-4">
              If an account exists with that email, a password reset link has been sent.
            </p>
            <Link to="/login" className="text-accent hover:text-accent-soft text-sm">
              Back to Login
            </Link>
          </div>
        ) : (
          <form onSubmit={handleSubmit} className="bg-surface border border-line rounded-2xl p-6 space-y-4">
            <input
              aria-label="Email address"
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              placeholder="Email address"
              className="w-full px-4 py-3 bg-raised/50 border border-line-strong rounded-lg text-fg placeholder-muted focus:outline-none focus:ring-2 focus:ring-purple-500"
              required
              autoFocus
            />
            <Button variant="primary" fullWidth type="submit" disabled={loading || !email.trim()}>
              {loading ? 'Sending…' : 'Send reset link'}
            </Button>
            <div className="text-center">
              <Link to="/login" className="text-accent hover:text-accent-soft text-sm">
                Back to Login
              </Link>
            </div>
          </form>
        )}
      </div>
    </div>
  );
}

function ResetForm({ token }: { token: string }) {
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState(false);
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    if (password.length < 8) {
      setError('Password must be at least 8 characters');
      return;
    }
    if (password !== confirm) {
      setError('Passwords do not match');
      return;
    }
    setLoading(true);
    setError(null);
    try {
      await api.typed((c) => c.POST('/auth/reset-password', { body: { token, password } }));
      setSuccess(true);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to reset password');
    }
    setLoading(false);
  };

  return (
    <div className="min-h-screen flex items-center justify-center bg-app px-4">
      <div className="w-full max-w-sm">
        <div className="text-center mb-8">
          <div className="w-12 h-12 rounded-2xl bg-purple-600 flex items-center justify-center text-2xl font-bold text-white mx-auto mb-4">
            C
          </div>
          <h1 className="text-2xl font-bold text-fg">Set new password</h1>
        </div>

        {success ? (
          <div className="bg-surface border border-line rounded-2xl p-6 text-center">
            <p className="text-success mb-4">Password has been reset successfully!</p>
            <Link to="/login" className={buttonStyles({ variant: 'primary' })}>
              Go to login
            </Link>
          </div>
        ) : (
          <form onSubmit={handleSubmit} className="bg-surface border border-line rounded-2xl p-6 space-y-4">
            <input
              aria-label="New password"
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder="New password"
              className="w-full px-4 py-3 bg-raised/50 border border-line-strong rounded-lg text-fg placeholder-muted focus:outline-none focus:ring-2 focus:ring-purple-500"
              required
              minLength={8}
              autoFocus
            />
            <input
              aria-label="Confirm new password"
              type="password"
              value={confirm}
              onChange={(e) => setConfirm(e.target.value)}
              placeholder="Confirm new password"
              className="w-full px-4 py-3 bg-raised/50 border border-line-strong rounded-lg text-fg placeholder-muted focus:outline-none focus:ring-2 focus:ring-purple-500"
              required
              minLength={8}
            />
            {error && <div className="text-danger text-sm">{error}</div>}
            <Button variant="primary" fullWidth type="submit" disabled={loading}>
              {loading ? 'Resetting…' : 'Reset password'}
            </Button>
          </form>
        )}
      </div>
    </div>
  );
}
