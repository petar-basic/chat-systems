import { useState } from 'react';
import { toUserMessage } from '@/lib/errors';
import { formatDateTime } from '@/lib/datetime';
import { X, Clock, Hash, Trash2, MessageSquare } from 'lucide-react';
import type { Channel } from '@/stores/workspace';
import type { Conversation } from '@/hooks/queries/useConversations';
import {
  useScheduledMessages,
  useCancelScheduledMessage,
  useRescheduleMessage,
  type ScheduledMessage,
} from '@/hooks/queries/useScheduledMessages';
import { targetLabel } from '@/lib/targetLabel';
import { toLocalInputValue } from '@/features/messaging/schedulePresets';
import { useUserCache } from '@/stores/users';
import { useWorkspaceStore } from '@/stores/workspace';

interface Props {
  workspaceId: string;
  instanceUrl?: string;
  channels: Channel[];
  conversations: Conversation[];
  onClose: () => void;
}

function ScheduledRow({
  scheduled,
  target,
  busy,
  onCancel,
  onReschedule,
}: {
  scheduled: ScheduledMessage;
  target: string;
  busy: boolean;
  onCancel: () => void;
  onReschedule: (sendAt: Date) => Promise<void>;
}) {
  const [editingTime, setEditingTime] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [earliest, setEarliest] = useState('');

  const sendAt = new Date(scheduled.send_at);

  return (
    <div
      className="px-4 py-3 border-b border-line/40"
      data-qa="scheduled-row"
      data-scheduled-id={scheduled.id}
    >
      <div className="flex items-start gap-2">
        <div className="flex-1 min-w-0">
          <div className="text-xs text-muted flex items-center gap-1.5" data-qa="scheduled-target">
            {scheduled.channel_id ? (
              <Hash className="w-3 h-3 shrink-0" />
            ) : (
              <MessageSquare className="w-3 h-3 shrink-0" />
            )}
            <span className="truncate">{target}</span>
          </div>
          <div className="text-sm text-fg-soft mt-0.5 line-clamp-2" data-qa="scheduled-content">
            {scheduled.content}
          </div>
          <div className="text-xs text-accent-soft mt-1" data-qa="scheduled-time">
            {formatDateTime(sendAt)}
          </div>
        </div>
        <button
          onClick={() => {
            if (editingTime !== null) {
              setEditingTime(null);
              return;
            }
            setEarliest(toLocalInputValue(new Date(Date.now() + 60_000)));
            setEditingTime(toLocalInputValue(sendAt));
          }}
          disabled={busy}
          aria-label={`Reschedule message for ${target}`}
          title="Reschedule"
          data-qa="scheduled-reschedule"
          className="p-1.5 rounded text-muted hover:text-fg hover:bg-raised transition cursor-pointer disabled:opacity-50"
        >
          <Clock className="w-4 h-4" />
        </button>
        <button
          onClick={onCancel}
          disabled={busy}
          aria-label={`Cancel message for ${target}`}
          title="Cancel"
          data-qa="scheduled-cancel"
          className="p-1.5 rounded text-muted hover:text-danger hover:bg-raised transition cursor-pointer disabled:opacity-50"
        >
          <Trash2 className="w-4 h-4" />
        </button>
      </div>

      {editingTime !== null && (
        <div className="mt-2">
          <input
            type="datetime-local"
            value={editingTime}
            min={earliest}
            onChange={(e) => {
              setEditingTime(e.target.value);
              setError(null);
            }}
            aria-label="New time"
            data-qa="scheduled-reschedule-input"
            className="w-full px-2 py-1.5 bg-raised/50 border border-line-strong rounded text-sm text-fg focus:outline-none focus:ring-2 focus:ring-purple-500 [color-scheme:dark]"
          />
          {error && (
            <p className="mt-1 text-[11px] text-danger" data-qa="scheduled-reschedule-error">
              {error}
            </p>
          )}
          <div className="mt-2 flex justify-end gap-2">
            <button
              onClick={() => setEditingTime(null)}
              className="px-2 py-1 text-xs text-muted hover:text-fg transition cursor-pointer"
            >
              Cancel
            </button>
            <button
              onClick={async () => {
                const at = new Date(editingTime);
                if (!editingTime || Number.isNaN(at.getTime())) {
                  setError('Pick a date and time first');
                  return;
                }
                if (at.getTime() <= Date.now()) {
                  setError('That time has already passed');
                  return;
                }
                setError(null);
                setSaving(true);
                try {
                  await onReschedule(at);
                  setEditingTime(null);
                } catch (err) {
                  setError(toUserMessage(err, 'Failed to move that message'));
                } finally {
                  setSaving(false);
                }
              }}
              disabled={saving}
              data-qa="scheduled-reschedule-submit"
              className="px-3 py-1 text-xs bg-purple-600 hover:bg-purple-500 text-white rounded transition cursor-pointer"
            >
              Move
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

export default function ScheduledMessagesPanel({
  workspaceId,
  instanceUrl,
  channels,
  conversations,
  onClose,
}: Props) {
  const { data: scheduled = [], isLoading } = useScheduledMessages(workspaceId, instanceUrl);
  const cancelScheduled = useCancelScheduledMessage(workspaceId, instanceUrl);
  const reschedule = useRescheduleMessage(workspaceId, instanceUrl);
  const [error, setError] = useState<string | null>(null);
  const { getUser } = useUserCache();
  const currentUserId = useWorkspaceStore((s) => s.currentUserId);

  const busy = cancelScheduled.isPending || reschedule.isPending;

  const targetOf = (message: ScheduledMessage) =>
    targetLabel(
      message.channel_id,
      channels,
      conversations,
      currentUserId ?? undefined,
      (id) => getUser(id)?.display_name,
    );

  const failWith = (fallback: string) => (err: unknown) =>
    setError((err as { message?: string })?.message || fallback);

  return (
    <div
      className="w-full lg:w-80 max-lg:fixed max-lg:inset-0 max-lg:z-40 bg-surface border-l border-line/50 flex flex-col"
      data-qa="scheduled-panel"
    >
      <div className="h-14 px-4 flex items-center justify-between border-b border-line/50 shrink-0">
        <div className="flex items-center gap-2">
          <Clock className="w-4 h-4 text-muted" />
          <span className="font-semibold">Scheduled</span>
        </div>
        <button onClick={onClose} className="text-muted hover:text-fg transition cursor-pointer">
          <X className="w-4 h-4" />
        </button>
      </div>

      <div className="flex-1 overflow-y-auto">
        {error && <div className="px-4 py-2 text-xs text-danger">{error}</div>}

        {isLoading ? (
          <div className="flex justify-center py-8">
            <div className="w-5 h-5 border-2 border-purple-500/30 border-t-purple-500 rounded-full animate-spin" />
          </div>
        ) : scheduled.length === 0 ? (
          <div className="text-center py-10 px-6 text-sm text-muted" data-qa="scheduled-empty">
            Nothing is waiting to be sent. Write a message, then pick the clock next to Send.
          </div>
        ) : (
          scheduled.map((message) => (
            <ScheduledRow
              key={message.id}
              scheduled={message}
              target={targetOf(message)}
              busy={busy}
              onCancel={() => {
                setError(null);
                cancelScheduled.mutate(message.id, { onError: failWith('Failed to cancel') });
              }}
              onReschedule={async (sendAt) => {
                setError(null);
                await reschedule.mutateAsync({ id: message.id, sendAt });
              }}
            />
          ))
        )}
      </div>
    </div>
  );
}
