import { X, History } from 'lucide-react';
import { useEditHistory } from '@/hooks/queries/useEditHistory';
import MessageContent from './MessageContent';

interface Props {
  messageId: string;
  scope: 'channel' | 'conversation';
  currentContent: string;
  onClose: () => void;
}

/**
 * Prior versions, newest replaced text first. Rendered through the same
 * renderer as the message itself so a version reads the way it read when it was
 * posted, rather than as raw markdown.
 */
export default function EditHistoryPanel({ messageId, scope, currentContent, onClose }: Props) {
  const { data: edits = [], isLoading, error } = useEditHistory(messageId, scope);

  return (
    <div className="mt-2 rounded-lg border border-slate-700 bg-slate-800/60 p-3" data-qa="edit-history">
      <div className="flex items-center gap-2 mb-2">
        <History className="w-3.5 h-3.5 text-slate-400" />
        <span className="text-xs font-semibold text-slate-300">Edit history</span>
        <button
          onClick={onClose}
          aria-label="Close edit history"
          className="ml-auto text-slate-400 hover:text-white transition cursor-pointer"
        >
          <X className="w-3.5 h-3.5" />
        </button>
      </div>

      {error ? (
        <p className="text-xs text-red-400">
          {error instanceof Error ? error.message : 'Could not load the history'}
        </p>
      ) : isLoading ? (
        <p className="text-xs text-slate-400">Loading…</p>
      ) : (
        <ol className="space-y-2">
          <li data-qa="edit-history-current">
            <div className="text-[11px] uppercase tracking-wider text-slate-500 mb-0.5">Now</div>
            <MessageContent content={currentContent} />
          </li>
          {edits.map((edit) => (
            <li key={edit.id} data-qa="edit-history-version" className="opacity-80">
              <div className="text-[11px] uppercase tracking-wider text-slate-500 mb-0.5">
                Before {new Date(edit.edited_at).toLocaleString()}
              </div>
              <MessageContent content={edit.previous_content} />
            </li>
          ))}
          {edits.length === 0 && <li className="text-xs text-slate-400">No earlier versions.</li>}
        </ol>
      )}
    </div>
  );
}
