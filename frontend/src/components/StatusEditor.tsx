import { useState } from 'react';
import { useMyStatus, useSetStatus } from '@/hooks/queries/useStatus';

interface Props {
  instanceUrl?: string;
  workspaceId?: string;
}

const PRESETS = [
  { emoji: '📅', text: 'In a meeting', minutes: 60 },
  { emoji: '🍕', text: 'At lunch', minutes: 60 },
  { emoji: '🤒', text: 'Out sick', minutes: 60 * 8 },
  { emoji: '🌴', text: 'On holiday', minutes: null },
];

const DURATIONS = [
  { label: "Don't clear", minutes: null },
  { label: '30 minutes', minutes: 30 },
  { label: '1 hour', minutes: 60 },
  { label: '4 hours', minutes: 240 },
  { label: 'Today', minutes: 60 * 12 },
];

export default function StatusEditor({ instanceUrl, workspaceId }: Props) {
  const { data: status } = useMyStatus(instanceUrl);
  const { set, clear } = useSetStatus(instanceUrl, workspaceId);

  const [emoji, setEmoji] = useState('');
  const [text, setText] = useState('');
  const [duration, setDuration] = useState<number | null>(null);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!loaded && status) {
    setLoaded(true);
    setEmoji(status.status_emoji ?? '');
    setText(status.status_text ?? '');
  }

  const save = async (next: { emoji: string; text: string; minutes: number | null }) => {
    setError(null);
    try {
      await set.mutateAsync({
        emoji: next.emoji || null,
        text: next.text || null,
        expiresAt: next.minutes ? new Date(Date.now() + next.minutes * 60_000) : null,
      });
    } catch (err) {
      setError((err as { message?: string })?.message || 'Failed to set that status');
    }
  };

  return (
    <div data-qa="status-editor">
      <span className="block text-sm font-medium text-fg-dim mb-1.5">Status</span>
      <div className="flex items-center gap-2">
        <input
          value={emoji}
          onChange={(e) => setEmoji(e.target.value)}
          placeholder="🙂"
          aria-label="Status emoji"
          data-qa="status-emoji"
          className="w-14 px-2 py-2.5 bg-raised/50 border border-line-strong rounded-lg text-fg text-sm text-center focus:outline-none focus:ring-2 focus:ring-purple-500"
        />
        <input
          value={text}
          onChange={(e) => setText(e.target.value)}
          placeholder="What's happening?"
          aria-label="Status"
          data-qa="status-text"
          className="flex-1 px-3 py-2.5 bg-raised/50 border border-line-strong rounded-lg text-fg text-sm placeholder-muted focus:outline-none focus:ring-2 focus:ring-purple-500"
        />
      </div>

      <div className="mt-2 flex flex-wrap gap-1.5">
        {PRESETS.map((preset) => (
          <button
            key={preset.text}
            type="button"
            onClick={() => {
              setEmoji(preset.emoji);
              setText(preset.text);
              setDuration(preset.minutes);
            }}
            data-qa="status-preset"
            className="px-2 py-1 rounded-md bg-raised/40 text-xs text-fg-dim hover:bg-raised transition cursor-pointer"
          >
            {preset.emoji} {preset.text}
          </button>
        ))}
      </div>

      <div className="mt-2 flex items-center gap-2">
        <select
          value={duration === null ? '' : String(duration)}
          onChange={(e) => setDuration(e.target.value === '' ? null : Number(e.target.value))}
          aria-label="Clear status after"
          data-qa="status-duration"
          className="px-2 py-1.5 bg-raised/50 border border-line-strong rounded-lg text-fg text-xs focus:outline-none focus:ring-2 focus:ring-purple-500"
        >
          {DURATIONS.map((option) => (
            <option key={option.label} value={option.minutes === null ? '' : String(option.minutes)}>
              {option.label}
            </option>
          ))}
        </select>
        <button
          type="button"
          onClick={() => save({ emoji: emoji.trim(), text: text.trim(), minutes: duration })}
          disabled={set.isPending || (!emoji.trim() && !text.trim())}
          data-qa="status-save"
          className="px-3 py-1.5 text-xs bg-purple-600 hover:bg-purple-500 text-white rounded-lg transition cursor-pointer disabled:opacity-50"
        >
          Set status
        </button>
        {(status?.status_text || status?.status_emoji) && (
          <button
            type="button"
            onClick={async () => {
              await clear.mutateAsync();
              setEmoji('');
              setText('');
              setDuration(null);
            }}
            disabled={clear.isPending}
            data-qa="status-clear"
            className="px-2 py-1.5 text-xs text-muted hover:text-danger transition cursor-pointer disabled:opacity-50"
          >
            Clear
          </button>
        )}
      </div>

      {status?.status_expires_at && (
        <p className="mt-1 text-[11px] text-muted" data-qa="status-expiry">
          Clears at {new Date(status.status_expires_at).toLocaleString()}
        </p>
      )}
      {error && (
        <p className="mt-1 text-[11px] text-danger" data-qa="status-error">
          {error}
        </p>
      )}
    </div>
  );
}
