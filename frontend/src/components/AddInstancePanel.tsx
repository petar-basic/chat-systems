import { useState, type FormEvent } from 'react';
import { Button } from '@/shared/components/Button/Button';
import { TextInput } from '@/shared/components/Form/TextInput';
import { PanelHeader } from '@/shared/components/PanelHeader/PanelHeader';
import { ServerCrash, LogIn } from 'lucide-react';
import { useInstanceStore } from '../stores/instances';
import { isTotpRequired } from '@/lib/errors';

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
  const [totpCode, setTotpCode] = useState('');
  const [needsTotp, setNeedsTotp] = useState(false);

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
      onClose();
    } catch (err) {
      if (isTotpRequired(err)) setNeedsTotp(true);
    }
  };

  return (
    <div className="absolute inset-0 z-30 flex">
      <div className="absolute inset-0 bg-overlay/50 backdrop-blur-sm" onClick={onClose} />

      <div className="relative z-10 w-80 bg-surface border-r border-line flex flex-col h-full shadow-2xl">
        <PanelHeader
          title="Add instance"
          icon={<ServerCrash className="h-4 w-4 text-accent" />}
          onClose={onClose}
        />

        <form onSubmit={handleSubmit} className="flex-1 overflow-y-auto p-4 space-y-4">
          <p className="text-xs text-muted">
            Connect to another Chat Systems server using its URL and your credentials.
          </p>

          {error && error !== 'totp_required' && (
            <div className="bg-red-500/10 border border-red-500/30 text-danger px-3 py-2 rounded-lg text-sm">
              {error}
            </div>
          )}

          {needsTotp && (
            <TextInput
              label="Authentication code"
              inputMode="text"
              autoComplete="one-time-code"
              value={totpCode}
              onChange={(e) => setTotpCode(e.target.value.trim())}
              placeholder="123456"
              required
            />
          )}

          <TextInput
            label="Server URL"
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
              <span>{showAdvanced ? '▾' : '▸'}</span> Advanced options
            </button>
            {showAdvanced && (
              <TextInput
                label="WebSocket URL"
                optional
                type="url"
                value={wsUrl}
                onChange={(e) => setWsUrl(e.target.value)}
                placeholder="ws://localhost:3004"
                className="mt-2"
              />
            )}
          </div>

          <TextInput
            label="Email"
            type="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            placeholder="you@company.com"
            autoComplete="email"
            required
          />

          <TextInput
            label="Password"
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            placeholder="Enter your password"
            autoComplete="current-password"
            required
          />

          <Button
            variant="primary"
            fullWidth
            type="submit"
            disabled={loading}
            loading={loading}
            icon={<LogIn className="h-4 w-4" />}
          >
            Connect
          </Button>
        </form>
      </div>
    </div>
  );
}
