import { useState, useEffect, type FormEvent } from 'react';
import { useNavigate } from 'react-router-dom';
import { ServerCrash, LogIn } from 'lucide-react';
import { useInstanceStore } from '../stores/instances';

export default function AddInstancePage() {
  const navigate = useNavigate();
  const { addInstance, instances, hydrated, loading, error, clearError } = useInstanceStore();

  const [url, setUrl] = useState(window.location.origin);
  const [wsUrl, setWsUrl] = useState('');
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');

  useEffect(() => {
    if (hydrated && instances.length > 0 && !url && !email && !password) {
      navigate('/app', { replace: true });
    }
  }, [hydrated, instances.length, navigate, url, email, password]);

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    clearError();
    try {
      await addInstance(url.trim(), email.trim(), password, wsUrl.trim() || undefined);
      navigate('/app', { replace: true });
    } catch {
      return;
    }
  };

  const isFirstInstance = instances.length === 0;

  return (
    <div className="min-h-screen bg-gradient-to-br from-slate-900 via-purple-900 to-slate-900 flex items-center justify-center p-4">
      <div className="w-full max-w-md">
        <div className="text-center mb-8">
          <div className="inline-flex items-center justify-center w-16 h-16 bg-purple-600 rounded-2xl mb-4">
            <ServerCrash className="w-8 h-8 text-white" />
          </div>
          <h1 className="text-3xl font-bold text-white">
            {isFirstInstance ? 'Welcome to Chat Systems' : 'Add Instance'}
          </h1>
          <p className="text-slate-400 mt-2">
            {isFirstInstance
              ? 'Connect to your Chat Systems server'
              : 'Connect to another Chat Systems server'}
          </p>
        </div>

        <form
          onSubmit={handleSubmit}
          className="bg-slate-800/50 backdrop-blur-xl border border-slate-700 rounded-2xl p-8 shadow-2xl"
        >
          {error && (
            <div className="bg-red-500/10 border border-red-500/30 text-red-400 px-4 py-3 rounded-lg mb-6 text-sm">
              {error}
            </div>
          )}

          <div className="mb-5">
            <label htmlFor="url" className="block text-sm font-medium text-slate-300 mb-2">
              Server URL
            </label>
            <input
              id="url"
              type="url"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              className="w-full px-4 py-3 bg-slate-700/50 border border-slate-600 rounded-lg text-white placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-purple-500 focus:border-transparent transition"
              placeholder="https://chat.yourcompany.com"
              required
            />
          </div>

          <div className="mb-5">
            <button
              type="button"
              onClick={() => setShowAdvanced(!showAdvanced)}
              className="text-xs text-slate-400 hover:text-slate-300 transition flex items-center gap-1"
            >
              <span>{showAdvanced ? '▾' : '▸'}</span> Advanced options
            </button>
            {showAdvanced && (
              <div className="mt-3">
                <label htmlFor="wsUrl" className="block text-sm font-medium text-slate-300 mb-2">
                  WebSocket URL
                  <span className="ml-2 text-xs text-slate-400 font-normal">
                    (optional — only needed if WS runs on a different port)
                  </span>
                </label>
                <input
                  id="wsUrl"
                  type="url"
                  value={wsUrl}
                  onChange={(e) => setWsUrl(e.target.value)}
                  className="w-full px-4 py-3 bg-slate-700/50 border border-slate-600 rounded-lg text-white placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-purple-500 focus:border-transparent transition"
                  placeholder="ws://localhost:3004"
                />
              </div>
            )}
          </div>

          <div className="mb-5">
            <label htmlFor="email" className="block text-sm font-medium text-slate-300 mb-2">
              Email
            </label>
            <input
              id="email"
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              className="w-full px-4 py-3 bg-slate-700/50 border border-slate-600 rounded-lg text-white placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-purple-500 focus:border-transparent transition"
              placeholder="you@company.com"
              required
            />
          </div>

          <div className="mb-6">
            <label htmlFor="password" className="block text-sm font-medium text-slate-300 mb-2">
              Password
            </label>
            <input
              id="password"
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              className="w-full px-4 py-3 bg-slate-700/50 border border-slate-600 rounded-lg text-white placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-purple-500 focus:border-transparent transition"
              placeholder="Enter your password"
              required
            />
          </div>

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
        </form>

        {!isFirstInstance && (
          <button
            onClick={() => navigate(-1)}
            className="mt-4 w-full text-center text-slate-400 text-sm hover:text-slate-300 transition"
          >
            Cancel
          </button>
        )}

        <p className="text-center text-slate-400 text-sm mt-6">
          Invite-only platform. Contact your admin for access.
        </p>
      </div>
    </div>
  );
}
