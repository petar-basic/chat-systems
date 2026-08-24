import { useState, useCallback, useRef, useEffect } from 'react';
import { api } from '../lib/api';
import { useUserCache } from '../stores/users';
import { useWorkspaceStore } from '../stores/workspace';
import type { Message } from '../stores/workspace';
import { X, Search } from 'lucide-react';
import { displayNameOf } from '@/lib/userHelpers';
import { Avatar } from '@/shared/components/Avatar/Avatar';
import { useEscapeToClose } from '@/shared/hooks/useEscapeToClose';

interface Props {
  onClose: () => void;
  onNavigateToMessage?: (channelId: string, messageId: string) => void;
  onNavigateToConversation?: (conversationId: string) => void;
}

interface ConversationHit {
  id: string;
  conversation_id: string;
  user_id: string;
  content: string;
  created_at: string;
}

function Hit({
  userId,
  content,
  createdAt,
  onClick,
  qa,
}: {
  userId: string;
  content: string;
  createdAt: string;
  onClick?: () => void;
  qa: string;
}) {
  const { getUser } = useUserCache();
  const sender = getUser(userId);
  const displayName = displayNameOf(sender?.display_name);

  return (
    <button
      type="button"
      onClick={onClick}
      data-qa={qa}
      className="w-full text-left px-3 py-2.5 hover:bg-slate-700/30 rounded-lg transition disabled:cursor-default"
      disabled={!onClick}
    >
      <div className="flex items-center gap-2 mb-0.5">
        <Avatar userId={userId} name={displayName} avatarUrl={sender?.avatar_url} size="xs" />
        <span className="text-sm font-semibold text-slate-200">{displayName}</span>
        <span className="text-xs text-slate-400">
          {new Date(createdAt).toLocaleDateString()}{' '}
          {new Date(createdAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
        </span>
      </div>
      <p className="text-sm text-slate-400 line-clamp-2">{content}</p>
    </button>
  );
}

export default function SearchPanel({ onClose, onNavigateToMessage, onNavigateToConversation }: Props) {
  useEscapeToClose(onClose);
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<Message[]>([]);
  const [conversationResults, setConversationResults] = useState<ConversationHit[]>([]);
  const [loading, setLoading] = useState(false);
  const [searched, setSearched] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const workspaceId = useWorkspaceStore((s) => s.currentWorkspace?.id);
  const inputRef = useRef<HTMLInputElement>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const doSearch = useCallback(
    async (q: string) => {
      if (!q.trim() || !workspaceId) {
        setResults([]);
        setConversationResults([]);
        setSearched(false);
        setError(null);
        return;
      }
      setLoading(true);
      setSearched(true);
      setError(null);
      try {
        const res = await api.get<{ data: Message[]; conversations: ConversationHit[] }>(
          `/search?q=${encodeURIComponent(q.trim())}&workspace_id=${workspaceId}&limit=20`,
        );
        setResults(res.data);
        setConversationResults(res.conversations ?? []);
      } catch (err: unknown) {
        setResults([]);
        setConversationResults([]);
        setError((err as { message?: string })?.message || 'Search failed');
      } finally {
        setLoading(false);
      }
    },
    [workspaceId],
  );

  const handleChange = (value: string) => {
    setQuery(value);
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => doSearch(value), 400);
  };

  return (
    <div className="w-full lg:w-80 max-lg:fixed max-lg:inset-0 max-lg:z-40 flex flex-col border-l border-slate-700/50 bg-slate-900/80">
      <div className="h-14 px-4 flex items-center justify-between border-b border-slate-700/50 shrink-0">
        <h3 className="text-sm font-bold text-white flex items-center gap-2">
          <Search className="w-4 h-4" />
          Search
        </h3>
        <button onClick={onClose} className="text-slate-400 hover:text-white transition cursor-pointer">
          <X className="w-4 h-4" />
        </button>
      </div>

      <div className="px-3 py-3 border-b border-slate-700/30">
        <div className="flex items-center gap-2 bg-slate-800 border border-slate-700 rounded-lg px-3 py-2">
          <Search className="w-4 h-4 text-slate-400 shrink-0" />
          <input
            ref={inputRef}
            type="text"
            value={query}
            onChange={(e) => handleChange(e.target.value)}
            placeholder="Search messages..."
            aria-label="Search messages"
            className="flex-1 bg-transparent text-white placeholder-slate-500 focus:outline-none text-sm"
          />
          {query && (
            <button
              onClick={() => {
                setQuery('');
                setResults([]);
                setConversationResults([]);
                setSearched(false);
              }}
              className="text-slate-400 hover:text-slate-300 transition cursor-pointer"
            >
              <X className="w-3.5 h-3.5" />
            </button>
          )}
        </div>
      </div>

      <div className="flex-1 overflow-y-auto px-2 py-2">
        {loading ? (
          <div className="flex justify-center py-8">
            <div className="w-5 h-5 border-2 border-purple-500/30 border-t-purple-500 rounded-full animate-spin" />
          </div>
        ) : error ? (
          <div className="text-center py-8 text-red-400 text-sm" data-qa="search-error">
            {error}
          </div>
        ) : searched && results.length === 0 && conversationResults.length === 0 ? (
          <div className="text-center py-8 text-slate-400 text-sm">
            No messages found for &ldquo;{query}&rdquo;
          </div>
        ) : results.length > 0 || conversationResults.length > 0 ? (
          <div className="space-y-4">
            {results.length > 0 && (
              <div className="space-y-1">
                {conversationResults.length > 0 && (
                  <h4 className="px-3 text-xs font-semibold uppercase tracking-wide text-slate-500">
                    Channels
                  </h4>
                )}
                {results.map((msg) => (
                  <Hit
                    key={msg.id}
                    qa="search-result"
                    userId={msg.user_id}
                    content={msg.content}
                    createdAt={msg.created_at}
                    onClick={
                      onNavigateToMessage ? () => onNavigateToMessage(msg.channel_id, msg.id) : undefined
                    }
                  />
                ))}
              </div>
            )}
            {conversationResults.length > 0 && (
              <div className="space-y-1">
                <h4 className="px-3 text-xs font-semibold uppercase tracking-wide text-slate-500">
                  Direct messages
                </h4>
                {conversationResults.map((hit) => (
                  <Hit
                    key={hit.id}
                    qa="search-result-conversation"
                    userId={hit.user_id}
                    content={hit.content}
                    createdAt={hit.created_at}
                    onClick={
                      onNavigateToConversation
                        ? () => onNavigateToConversation(hit.conversation_id)
                        : undefined
                    }
                  />
                ))}
              </div>
            )}
          </div>
        ) : (
          <div className="text-center py-8 text-slate-400 text-sm">
            Search across all messages in this workspace
          </div>
        )}
      </div>
    </div>
  );
}
