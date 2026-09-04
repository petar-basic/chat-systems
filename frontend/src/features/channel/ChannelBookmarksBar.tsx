import { useState } from 'react';
import { toUserMessage } from '@/lib/errors';
import { Plus, X, ExternalLink } from 'lucide-react';
import {
  useChannelBookmarks,
  useCreateChannelBookmark,
  useDeleteChannelBookmark,
} from '@/hooks/queries/useChannelBookmarks';
import { useChannelModeration } from './hooks/useChannelModeration';

interface Props {
  channelId: string;
  instanceUrl?: string;
}

export default function ChannelBookmarksBar({ channelId, instanceUrl }: Props) {
  const { data: bookmarks = [] } = useChannelBookmarks(channelId, instanceUrl);
  const { canModerate } = useChannelModeration(channelId);
  const createBookmark = useCreateChannelBookmark(channelId, instanceUrl);
  const deleteBookmark = useDeleteChannelBookmark(channelId, instanceUrl);

  const [adding, setAdding] = useState(false);
  const [label, setLabel] = useState('');
  const [url, setUrl] = useState('');
  const [error, setError] = useState<string | null>(null);

  if (bookmarks.length === 0 && !canModerate) return null;

  const submit = async () => {
    const trimmedLabel = label.trim();
    const trimmedUrl = url.trim();
    if (!trimmedLabel || !trimmedUrl) {
      setError('A bookmark needs a label and a link');
      return;
    }
    setError(null);
    try {
      await createBookmark.mutateAsync({ label: trimmedLabel, url: trimmedUrl });
      setLabel('');
      setUrl('');
      setAdding(false);
    } catch (err) {
      setError(toUserMessage(err, 'Failed to add that bookmark'));
    }
  };

  return (
    <div
      // An empty bar still costs a row, and on a phone that row is expensive.
      // With nothing pinned here, the invitation to add one waits for a wider
      // screen; the channel settings reach it either way.
      className={`px-4 py-1.5 flex items-center gap-2 flex-wrap border-b border-line/50 bg-surface/20 shrink-0 ${
        bookmarks.length === 0 ? 'max-sm:hidden' : ''
      }`}
      data-qa="channel-bookmarks"
    >
      {bookmarks.map((bookmark) => (
        <span
          key={bookmark.id}
          className="group inline-flex items-center gap-1 pl-2 pr-1 py-0.5 rounded-md bg-raised/40 text-xs text-fg-soft"
          data-qa="channel-bookmark"
          data-bookmark-id={bookmark.id}
        >
          <a
            href={bookmark.url}
            target="_blank"
            rel="noreferrer noopener"
            className="inline-flex items-center gap-1 hover:text-fg transition"
          >
            {bookmark.emoji ? <span>{bookmark.emoji}</span> : <ExternalLink className="w-3 h-3" />}
            <span className="max-w-40 truncate">{bookmark.label}</span>
          </a>
          {canModerate && (
            <button
              onClick={() => deleteBookmark.mutate(bookmark.id)}
              aria-label={`Remove bookmark ${bookmark.label}`}
              data-qa="channel-bookmark-remove"
              className="p-0.5 rounded text-subtle hover:text-danger transition cursor-pointer opacity-0 group-hover:opacity-100 focus:opacity-100"
            >
              <X className="w-3 h-3" />
            </button>
          )}
        </span>
      ))}

      {canModerate && !adding && (
        <button
          onClick={() => setAdding(true)}
          data-qa="channel-bookmark-add"
          className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded-md text-xs text-muted hover:text-fg hover:bg-raised/50 transition cursor-pointer"
        >
          <Plus className="w-3 h-3" />
          Add a bookmark
        </button>
      )}

      {adding && (
        <div className="flex items-center gap-1.5 flex-wrap">
          <input
            value={label}
            onChange={(e) => setLabel(e.target.value)}
            placeholder="Label"
            aria-label="Bookmark label"
            data-qa="channel-bookmark-label"
            className="px-2 py-1 w-32 bg-raised/50 border border-line-strong rounded text-xs text-fg placeholder-muted focus:outline-none focus:ring-2 focus:ring-purple-500"
          />
          <input
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            placeholder="https://…"
            aria-label="Bookmark link"
            data-qa="channel-bookmark-url"
            className="px-2 py-1 w-56 bg-raised/50 border border-line-strong rounded text-xs text-fg placeholder-muted focus:outline-none focus:ring-2 focus:ring-purple-500"
          />
          <button
            onClick={submit}
            disabled={createBookmark.isPending}
            data-qa="channel-bookmark-save"
            className="px-2 py-1 text-xs bg-purple-600 hover:bg-purple-500 text-white rounded transition cursor-pointer disabled:opacity-50"
          >
            Add
          </button>
          <button
            onClick={() => {
              setAdding(false);
              setError(null);
            }}
            className="px-2 py-1 text-xs text-muted hover:text-fg transition cursor-pointer"
          >
            Cancel
          </button>
          {error && (
            <span className="text-[11px] text-danger" data-qa="channel-bookmark-error">
              {error}
            </span>
          )}
        </div>
      )}
    </div>
  );
}
