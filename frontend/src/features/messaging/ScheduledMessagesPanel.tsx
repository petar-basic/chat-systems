import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Spinner } from '@/shared/components/Spinner/Spinner';
import { Button } from '@/shared/components/Button/Button';
import { PanelHeader } from '@/shared/components/PanelHeader/PanelHeader';
import { toUserMessage } from '@/lib/errors';
import { formatDateTime } from '@/lib/datetime';
import { AlertCircle, Clock, Hash, Trash2, MessageSquare } from 'lucide-react';
import type { Channel } from '@/stores/workspace';
import type { Conversation } from '@/features/messaging/hooks/useConversations';
import {
  useScheduledMessages,
  useCancelScheduledMessage,
  useRescheduleMessage,
  useSendScheduledNow,
  type ScheduledMessage,
} from '@/features/messaging/hooks/useScheduledMessages';
import { targetLabel } from '@/lib/targetLabel';
import { toLocalInputValue } from '@/features/messaging/schedulePresets';
import { useUserCache } from '@/stores/users';
import { useWorkspaceStore } from '@/stores/workspace';
import { TextInput } from '@/shared/components/Form/TextInput';
import { failureKey } from '@/lib/scheduledFailure';

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
  onSendNow,
}: {
  scheduled: ScheduledMessage;
  target: string;
  busy: boolean;
  onCancel: () => void;
  onReschedule: (sendAt: Date) => Promise<void>;
  onSendNow: () => void;
}) {
  const { t } = useTranslation(['messages', 'common']);
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
          {scheduled.attachment_ids.length > 0 && (
            <div className="text-xs text-muted mt-0.5" data-qa="scheduled-files">
              📎 {t('scheduled.files', { count: scheduled.attachment_ids.length })}
            </div>
          )}
          {scheduled.failure ? (
            <div className="text-xs text-danger mt-1 flex items-start gap-1" data-qa="scheduled-failure">
              <AlertCircle className="w-3.5 h-3.5 shrink-0 mt-px" />
              <span>
                {t('scheduled.notSent', { reason: t(`scheduled.reasons.${failureKey(scheduled.failure)}`) })}
              </span>
            </div>
          ) : (
            <div className="text-xs text-accent-soft mt-1" data-qa="scheduled-time">
              {formatDateTime(sendAt)}
            </div>
          )}
        </div>
        {!scheduled.failure && (
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
            aria-label={t('scheduled.rescheduleLabel', { target })}
            data-tooltip={t('scheduled.reschedule')}
            data-qa="scheduled-reschedule"
            className="p-1.5 rounded text-muted hover:text-fg hover:bg-raised transition cursor-pointer disabled:opacity-50"
          >
            <Clock className="w-4 h-4" />
          </button>
        )}
        <button
          onClick={onCancel}
          disabled={busy}
          aria-label={
            scheduled.failure
              ? t('scheduled.removeLabel', { target })
              : t('scheduled.cancelLabel', { target })
          }
          data-tooltip={scheduled.failure ? t('scheduled.remove') : t('scheduled.cancel')}
          data-qa="scheduled-cancel"
          className="p-1.5 rounded text-muted hover:text-danger hover:bg-raised transition cursor-pointer disabled:opacity-50"
        >
          <Trash2 className="w-4 h-4" />
        </button>
      </div>
      {scheduled.failure && (
        <div className="mt-2 flex justify-end">
          <Button
            variant="secondary"
            size="sm"
            onClick={onSendNow}
            disabled={busy}
            data-qa="scheduled-send-now"
          >
            {t('scheduled.sendNow')}
          </Button>
        </div>
      )}

      {editingTime !== null && (
        <div className="mt-2">
          <TextInput
            label={t('scheduled.newTime')}
            labelHidden
            compact
            type="datetime-local"
            value={editingTime}
            min={earliest}
            onChange={(e) => {
              setEditingTime(e.target.value);
              setError(null);
            }}
            dataQa="scheduled-reschedule-input"
          />
          {error && (
            <p className="mt-1 text-2xs text-danger" data-qa="scheduled-reschedule-error">
              {error}
            </p>
          )}
          <div className="mt-2 flex justify-end gap-2">
            <button
              onClick={() => setEditingTime(null)}
              className="px-2 py-1 text-xs text-muted hover:text-fg transition cursor-pointer"
            >
              {t('cancel', { ns: 'common' })}
            </button>
            <Button
              variant="primary"
              size="sm"
              onClick={async () => {
                const at = new Date(editingTime);
                if (!editingTime || Number.isNaN(at.getTime())) {
                  setError(t('schedule.needTime'));
                  return;
                }
                if (at.getTime() <= Date.now()) {
                  setError(t('schedule.past'));
                  return;
                }
                setError(null);
                setSaving(true);
                try {
                  await onReschedule(at);
                  setEditingTime(null);
                } catch (err) {
                  setError(toUserMessage(err, t('scheduled.moveFailed')));
                } finally {
                  setSaving(false);
                }
              }}
              disabled={saving}
              data-qa="scheduled-reschedule-submit"
            >
              {t('scheduled.move')}
            </Button>
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
  const { t } = useTranslation('messages');
  const { data: scheduled = [], isLoading } = useScheduledMessages(workspaceId, instanceUrl);
  const cancelScheduled = useCancelScheduledMessage(workspaceId, instanceUrl);
  const reschedule = useRescheduleMessage(workspaceId, instanceUrl);
  const sendNow = useSendScheduledNow(workspaceId, instanceUrl);
  const [error, setError] = useState<string | null>(null);
  const { getUser } = useUserCache();
  const currentUserId = useWorkspaceStore((s) => s.currentUserId);

  const busy = cancelScheduled.isPending || reschedule.isPending || sendNow.isPending;

  const targetOf = (message: ScheduledMessage) => {
    const label = targetLabel(
      message.channel_id,
      channels,
      conversations,
      currentUserId ?? undefined,
      (id) => getUser(id)?.display_name,
      t('scheduled.leftChannel'),
    );
    return message.thread_parent_id ? t('scheduled.thread', { target: label }) : label;
  };

  const failWith = (fallback: string) => (err: unknown) =>
    setError((err as { message?: string })?.message || fallback);

  return (
    <div
      className="w-full lg:w-80 max-lg:fixed max-lg:inset-0 max-lg:z-40 bg-surface border-l border-line/50 flex flex-col"
      data-qa="scheduled-panel"
    >
      <PanelHeader
        title={t('scheduled.title')}
        icon={<Clock className="h-4 w-4 text-muted" />}
        onClose={onClose}
      />

      <div className="flex-1 overflow-y-auto">
        {error && <div className="px-4 py-2 text-xs text-danger">{error}</div>}

        {isLoading ? (
          <div className="flex justify-center py-8">
            <Spinner className="text-accent" />
          </div>
        ) : scheduled.length === 0 ? (
          <div className="text-center py-10 px-6 text-sm text-muted" data-qa="scheduled-empty">
            {t('scheduled.empty')}
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
                cancelScheduled.mutate(message.id, { onError: failWith(t('scheduled.cancelFailed')) });
              }}
              onReschedule={async (sendAt) => {
                setError(null);
                await reschedule.mutateAsync({ id: message.id, sendAt });
              }}
              onSendNow={() => {
                setError(null);
                sendNow.mutate(message.id, { onError: failWith(t('scheduled.sendFailed')) });
              }}
            />
          ))
        )}
      </div>
    </div>
  );
}
