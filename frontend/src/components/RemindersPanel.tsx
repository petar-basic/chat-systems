import { useState } from 'react';
import { X, BellRing, Hash, Trash2 } from 'lucide-react';
import type { Channel } from '@/stores/workspace';
import { useReminders, useCreateReminder, useCancelReminder } from '@/hooks/queries/useReminders';
import { toLocalInputValue } from '@/features/messaging/schedulePresets';

interface Props {
  workspaceId: string;
  instanceUrl?: string;
  channels: Channel[];
  currentUserId: string;
  onClose: () => void;
}

export default function RemindersPanel({
  workspaceId,
  instanceUrl,
  channels,
  currentUserId,
  onClose,
}: Props) {
  const { data: reminders = [], isLoading } = useReminders(workspaceId, instanceUrl);
  const createReminder = useCreateReminder(workspaceId, instanceUrl);
  const cancelReminder = useCancelReminder(workspaceId, instanceUrl);

  const [content, setContent] = useState('');
  const [remindAt, setRemindAt] = useState(() => toLocalInputValue(new Date(Date.now() + 60 * 60 * 1000)));
  const [earliest] = useState(() => toLocalInputValue(new Date(Date.now() + 60_000)));
  const [error, setError] = useState<string | null>(null);

  const pending = reminders
    .filter((reminder) => !reminder.is_delivered)
    .sort((a, b) => a.remind_at.localeCompare(b.remind_at));

  const submit = async () => {
    const trimmed = content.trim();
    const at = new Date(remindAt);
    if (!trimmed) {
      setError('Say what to remind you about');
      return;
    }
    if (Number.isNaN(at.getTime()) || at.getTime() <= Date.now()) {
      setError('Pick a time in the future');
      return;
    }
    setError(null);
    try {
      await createReminder.mutateAsync({ targetUserId: currentUserId, content: trimmed, remindAt: at });
      setContent('');
    } catch (err) {
      setError((err as { message?: string })?.message || 'Failed to set that reminder');
    }
  };

  return (
    <div
      className="w-full lg:w-80 max-lg:fixed max-lg:inset-0 max-lg:z-40 bg-surface border-l border-line/50 flex flex-col"
      data-qa="reminders-panel"
    >
      <div className="h-14 px-4 flex items-center justify-between border-b border-line/50 shrink-0">
        <div className="flex items-center gap-2">
          <BellRing className="w-4 h-4 text-muted" />
          <span className="font-semibold">Reminders</span>
        </div>
        <button
          onClick={onClose}
          aria-label="Close reminders"
          className="text-muted hover:text-fg transition cursor-pointer"
        >
          <X className="w-4 h-4" />
        </button>
      </div>

      <div className="px-4 py-3 border-b border-line/40 space-y-2">
        <input
          value={content}
          onChange={(e) => setContent(e.target.value)}
          placeholder="Remind me to…"
          aria-label="Reminder"
          data-qa="reminder-content"
          className="w-full px-2 py-1.5 bg-raised/50 border border-line-strong rounded text-sm text-fg placeholder-muted focus:outline-none focus:ring-2 focus:ring-purple-500"
        />
        <input
          type="datetime-local"
          value={remindAt}
          min={earliest}
          onChange={(e) => setRemindAt(e.target.value)}
          aria-label="Remind at"
          data-qa="reminder-time"
          className="w-full px-2 py-1.5 bg-raised/50 border border-line-strong rounded text-sm text-fg focus:outline-none focus:ring-2 focus:ring-purple-500 [color-scheme:dark]"
        />
        {error && (
          <p className="text-[11px] text-danger" data-qa="reminder-error">
            {error}
          </p>
        )}
        <div className="flex items-center justify-between">
          <span className="text-[11px] text-muted">or type /remind me in 30m to…</span>
          <button
            onClick={submit}
            disabled={createReminder.isPending}
            data-qa="reminder-submit"
            className="px-3 py-1 text-xs bg-purple-600 hover:bg-purple-500 text-white rounded transition cursor-pointer disabled:opacity-50"
          >
            Remind me
          </button>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto">
        {isLoading ? (
          <div className="flex justify-center py-8">
            <div className="w-5 h-5 border-2 border-purple-500/30 border-t-purple-500 rounded-full animate-spin" />
          </div>
        ) : pending.length === 0 ? (
          <div className="text-center py-10 px-6 text-sm text-muted" data-qa="reminders-empty">
            Nothing is waiting for you.
          </div>
        ) : (
          pending.map((reminder) => {
            const channel = channels.find((c) => c.id === reminder.channel_id);
            return (
              <div
                key={reminder.id}
                className="px-4 py-3 border-b border-line/40 flex items-start gap-2"
                data-qa="reminder-row"
                data-reminder-id={reminder.id}
              >
                <div className="flex-1 min-w-0">
                  <div className="text-sm text-fg-soft line-clamp-2" data-qa="reminder-text">
                    {reminder.content}
                  </div>
                  <div className="text-xs text-accent-soft mt-1" data-qa="reminder-at">
                    {new Date(reminder.remind_at).toLocaleString()}
                  </div>
                  {channel && (
                    <div className="text-xs text-muted mt-0.5 flex items-center gap-1">
                      <Hash className="w-3 h-3 shrink-0" />
                      <span className="truncate">{channel.name}</span>
                    </div>
                  )}
                </div>
                <button
                  onClick={() => cancelReminder.mutate(reminder.id)}
                  disabled={cancelReminder.isPending}
                  aria-label="Cancel reminder"
                  title="Cancel"
                  data-qa="reminder-cancel"
                  className="p-1.5 rounded text-muted hover:text-danger hover:bg-raised transition cursor-pointer disabled:opacity-50"
                >
                  <Trash2 className="w-4 h-4" />
                </button>
              </div>
            );
          })
        )}
      </div>
    </div>
  );
}
