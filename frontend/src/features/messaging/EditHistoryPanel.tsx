import { X, History } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { formatDateTime } from '@/lib/datetime';
import { useEditHistory } from '@/features/messaging/hooks/useEditHistory';
import MessageContent from './MessageContent';

interface Props {
  messageId: string;
  currentContent: string;
  onClose: () => void;
}

/**
 * Prior versions, newest replaced text first. Rendered through the same
 * renderer as the message itself so a version reads the way it read when it was
 * posted, rather than as raw markdown.
 */
export default function EditHistoryPanel({ messageId, currentContent, onClose }: Props) {
  const { t } = useTranslation('messages');
  const { data: edits = [], isLoading, error } = useEditHistory(messageId);

  return (
    <div className="mt-2 rounded-lg border border-line bg-surface/60 p-3" data-qa="edit-history">
      <div className="flex items-center gap-2 mb-2">
        <History className="w-3.5 h-3.5 text-muted" />
        <span className="text-xs font-semibold text-fg-dim">{t('history.title')}</span>
        <button
          onClick={onClose}
          aria-label={t('history.close')}
          data-tooltip={t('history.close')}
          className="ml-auto text-muted hover:text-fg transition cursor-pointer"
        >
          <X className="w-3.5 h-3.5" />
        </button>
      </div>

      {error ? (
        <p className="text-xs text-danger">{error instanceof Error ? error.message : t('history.failed')}</p>
      ) : isLoading ? (
        <p className="text-xs text-muted">{t('history.loading')}</p>
      ) : (
        <ol className="space-y-2">
          <li data-qa="edit-history-current">
            <div className="text-2xs uppercase tracking-wider text-subtle mb-0.5">{t('history.now')}</div>
            <MessageContent content={currentContent} />
          </li>
          {edits.map((edit) => (
            <li key={edit.id} data-qa="edit-history-version" className="opacity-80">
              <div className="text-2xs uppercase tracking-wider text-subtle mb-0.5">
                {t('history.before', { time: formatDateTime(edit.edited_at) })}
              </div>
              <MessageContent content={edit.previous_content} />
            </li>
          ))}
          {edits.length === 0 && <li className="text-xs text-muted">{t('history.none')}</li>}
        </ol>
      )}
    </div>
  );
}
