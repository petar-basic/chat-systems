import { useState, type FormEvent } from 'react';
import { X, ServerCrash, LogIn } from 'lucide-react';
import { useInstanceStore } from '../stores/instances';

interface Props {
  onClose: () => void;
}

export default function AddInstancePanel({ onClose }: Props) {
  const { addInstance, loading, error, clearError } = useInstanceStore();
  const [url, setUrl] = useState('');
  const [wsUrl, setWsUrl] = useState('');
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    clearError();
    try {
      await addInstance(url.trim(), email.trim(), password, wsUrl.trim() || undefined);
      onClose();
    } catch {
      return;
    }
  };

  return (
    <div className="absolute inset-0 z-30 flex">
      <div className="absolute inset-0 bg-black/50 backdrop-blur-sm" onClick={onClose} />

      <div className="relative z-10 w-80 bg-slate-800 border-r border-slate-700 flex flex-col h-full shadow-2xl">
        <div className="h-14 px-4 flex items-center justify-between border-b border-slate-700 shrink-0">
          <div className="flex items-center gap-2">
            <ServerCrash className="w-5 h-5 text-purple-400" />
            <h2 className="font-semibold text-white">Add Instance</h2>
          </div>
          <button onClick={onClose} className="text-slate-400 hover:text-white transition cursor-pointer">
            <X className="w-5 h-5" />
          </button>
        </div>

        <form onSubmit={handleSubmit} className="flex-1 overflow-y-auto p-4 space-y-4">
          <p className="text-xs text-slate-400">
            Connect to another Chat Systems server using its URL and your credentials.
          </p>

          {error && (
            <div className="bg-red-500/10 border border-red-500/30 text-red-400 px-3 py-2 rounded-lg text-sm">
              {error}
            </div>
          )}

          <div>
            <label htmlFor="instance-url" className="block text-sm font-medium text-slate-300 mb-1.5">
              Server URL
            </label>
            <input
              id="instance-url"
              type="url"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              className="w-full px-3 py-2.5 bg-slate-700/50 border border-slate-600 rounded-lg text-white text-sm placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-purple-500"
              placeholder="https://chat.yourcompany.com"
              required
            />
          </div>

          <div>
            <button
              type="button"
              onClick={() => setShowAdvanced(!showAdvanced)}
              className="text-xs text-slate-400 hover:text-slate-300 transition flex items-center gap-1"
            >
              <span>{showAdvanced ? '▾' : '▸'}</span> Advanced options
            </button>
            {showAdvanced && (
              <div className="mt-2">
                <label htmlFor="instance-ws-url" className="block text-sm font-medium text-slate-300 mb-1.5">
                  WebSocket URL
                  <span className="ml-2 text-xs text-slate-400 font-normal">(optional)</span>
                </label>
                <input
                  id="instance-ws-url"
                  type="url"
                  value={wsUrl}
                  onChange={(e) => setWsUrl(e.target.value)}
                  className="w-full px-3 py-2.5 bg-slate-700/50 border border-slate-600 rounded-lg text-white text-sm placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-purple-500"
                  placeholder="ws://localhost:3004"
                />
              </div>
            )}
          </div>

          <div>
            <label htmlFor="instance-email" className="block text-sm font-medium text-slate-300 mb-1.5">
              Email
            </label>
            <input
              id="instance-email"
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              className="w-full px-3 py-2.5 bg-slate-700/50 border border-slate-600 rounded-lg text-white text-sm placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-purple-500"
              placeholder="you@company.com"
              required
            />
          </div>

          <div>
            <label htmlFor="instance-password" className="block text-sm font-medium text-slate-300 mb-1.5">
              Password
            </label>
            <input
              id="instance-password"
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              className="w-full px-3 py-2.5 bg-slate-700/50 border border-slate-600 rounded-lg text-white text-sm placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-purple-500"
              placeholder="Enter your password"
              required
            />
          </div>

          <button
            type="submit"
            disabled={loading}
            className="w-full flex items-center justify-center gap-2 px-3 py-2.5 bg-purple-600 hover:bg-purple-500 disabled:bg-purple-600/50 text-white text-sm font-medium rounded-lg transition cursor-pointer disabled:cursor-not-allowed"
          >
            {loading ? (
              <div className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin" />
            ) : (
              <>
                <LogIn className="w-4 h-4" />
                Connect
              </>
            )}
          </button>
        </form>
      </div>
    </div>
  );
}
