import { useState, useCallback, useRef, useEffect } from 'react';
import { Spinner } from '@/shared/components/Spinner/Spinner';
import { PanelHeader } from '@/shared/components/PanelHeader/PanelHeader';
import { toUserMessage } from '@/lib/errors';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { useUserCache } from '../stores/users';
import { useWorkspaceStore } from '../stores/workspace';
import type { Message, WorkspaceMember } from '../stores/workspace';
import { SearchFilterBar } from '@/features/search/SearchFilterBar';
import {
  extractFilters,
  hasFilters,
  searchParams,
  type FilterContext,
  type SearchFilters,
} from '@/features/search/searchFilters';
import { X, Search } from 'lucide-react';
import { IconButton } from '@/shared/components/IconButton/IconButton';
import { displayNameOf } from '@/lib/userHelpers';
import { Avatar } from '@/shared/components/Avatar/Avatar';
import { useEscapeToClose } from '@/shared/hooks/useEscapeToClose';
import { useWorkspaceChannels } from '@/hooks/queries/useWorkspaces';
import { useConversations } from '@/hooks/queries/useConversations';
import { toPlainText } from '@/lib/plainText';
import { formatDateTime } from '@/lib/datetime';
import { TextInput } from '@/shared/components/Form/TextInput';

interface Props {
  members: WorkspaceMember[];
  onClose: () => void;
  onNavigateToMessage?: (channelId: string, messageId: string) => void;
  onNavigateToConversation?: (conversationId: string) => void;
}

function Hit({
  userId,
  content,
  createdAt,
  context,
  onClick,
  qa,
}: {
  userId: string;
  content: string;
  createdAt: string;
  context?: string;
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
      className="w-full text-left px-3 py-2.5 hover:bg-raised/30 rounded-lg transition disabled:cursor-default"
      disabled={!onClick}
    >
      <div className="flex items-center gap-2 mb-0.5 min-w-0">
        <Avatar userId={userId} name={displayName} avatarUrl={sender?.avatar_url} size="xs" />
        <span className="text-sm font-semibold text-fg-soft">{displayName}</span>
        {context && (
          <span className="text-xs text-accent truncate" data-qa="search-result-context">
            {context}
          </span>
        )}
        <span className="text-xs text-muted shrink-0">{formatDateTime(createdAt)}</span>
      </div>
      <p className="text-sm text-muted line-clamp-2">{toPlainText(content)}</p>
    </button>
  );
}

export default function SearchPanel({
  members,
  onClose,
  onNavigateToMessage,
  onNavigateToConversation,
}: Props) {
  useEscapeToClose(onClose);
  const [query, setQuery] = useState('');
  const [filters, setFilters] = useState<SearchFilters>({});
  const [results, setResults] = useState<Message[]>([]);
  const [loading, setLoading] = useState(false);
  const [searched, setSearched] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const workspaceId = useWorkspaceStore((s) => s.currentWorkspace?.id);
  const instanceUrl = useWorkspaceStore((s) => s.currentWorkspace?.instanceUrl);
  const currentUserId = useWorkspaceStore((s) => s.currentUserId);
  const { data: channels = [] } = useWorkspaceChannels(workspaceId ?? null, instanceUrl);
  const { data: conversations = [] } = useConversations(workspaceId ?? null, instanceUrl);
  const [today] = useState(() => new Date());
  const context: FilterContext = { members, channels, conversations, currentUserId, today };
  const channelNames = new Map(channels.map((ch) => [ch.id, ch.name]));
  const conversationIds = new Set(conversations.map((c) => c.id));
  const inputRef = useRef<HTMLInputElement>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const requestRef = useRef(0);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const doSearch = useCallback(
    async (q: string, active: SearchFilters) => {
      const request = ++requestRef.current;
      if ((!q.trim() && !hasFilters(active)) || !workspaceId) {
        setResults([]);
        setSearched(false);
        setError(null);
        setLoading(false);
        return;
      }
      setLoading(true);
      setSearched(true);
      setError(null);
      try {
        const res = await getApiForInstance(instanceUrl).typed((c) =>
          c.GET('/search', {
            params: {
              query: { q: q.trim(), workspace_id: workspaceId, limit: 20, ...searchParams(active) },
            },
          }),
        );
        if (request === requestRef.current) setResults(res.data);
      } catch (err: unknown) {
        if (request !== requestRef.current) return;
        setResults([]);
        setError(toUserMessage(err, 'Search failed'));
      } finally {
        if (request === requestRef.current) setLoading(false);
      }
    },
    [workspaceId, instanceUrl],
  );

  const search = (text: string, active: SearchFilters, delay: number) => {
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => void doSearch(text, active), delay);
  };

  const apply = (text: string, final: boolean) => {
    const extracted = extractFilters(text, context, final);
    const nextText = extracted?.text ?? text;
    const nextFilters = extracted ? { ...filters, ...extracted.filters } : filters;
    setQuery(nextText);
    if (extracted) setFilters(nextFilters);
    search(nextText, nextFilters, final || extracted ? 0 : 400);
  };

  const changeFilters = (next: SearchFilters) => {
    setFilters(next);
    search(query, next, 0);
  };

  const filtered = hasFilters(filters);

  return (
    <div className="w-full lg:w-80 max-lg:fixed max-lg:inset-0 max-lg:z-40 flex flex-col border-l border-line/50 bg-app lg:bg-app/80">
      <PanelHeader title="Search" icon={<Search className="h-4 w-4 text-muted" />} onClose={onClose} />

      <div className="px-3 py-3 border-b border-line/30 space-y-2">
        <TextInput
          label="Search messages"
          labelHidden
          ref={inputRef}
          value={query}
          onChange={(e) => apply(e.target.value, false)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') apply(query, true);
          }}
          placeholder="Search messages..."
          title="Narrow with from:@name in:#channel has:file before:2026-10-01 after: on:"
          leading={<Search className="w-4 h-4" />}
          trailing={
            query && (
              <IconButton
                label="Clear search"
                icon={<X className="w-3.5 h-3.5" />}
                onClick={() => {
                  setQuery('');
                  search('', filters, 0);
                }}
              />
            )
          }
        />
        <SearchFilterBar filters={filters} onChange={changeFilters} context={context} />
      </div>

      {searched && !loading && !error && (
        <div
          className="flex items-center justify-between px-4 pt-2 text-xs text-muted"
          data-qa="search-summary"
        >
          <span>
            {results.length === 20 ? '20+' : results.length} {results.length === 1 ? 'result' : 'results'}
          </span>
          {filtered && (
            <button
              type="button"
              onClick={() => changeFilters({})}
              data-qa="search-clear-filters"
              className="text-accent hover:text-accent-soft cursor-pointer"
            >
              Clear filters
            </button>
          )}
        </div>
      )}

      <div className="flex-1 overflow-y-auto px-2 py-2">
        {loading ? (
          <div className="flex justify-center py-8">
            <Spinner className="text-accent" />
          </div>
        ) : error ? (
          <div className="text-center py-8 text-danger text-sm" data-qa="search-error">
            {error}
          </div>
        ) : searched && results.length === 0 ? (
          <div className="text-center py-8 text-muted text-sm">
            {query.trim() ? (
              <>
                No messages found for &ldquo;{query.trim()}&rdquo;{filtered && ' with these filters'}
              </>
            ) : (
              'No messages match these filters'
            )}
          </div>
        ) : results.length > 0 ? (
          <div className="space-y-1">
            {results.map((msg) => {
              const conversation = conversationIds.has(msg.channel_id);
              return (
                <Hit
                  key={msg.id}
                  qa={conversation ? 'search-result-conversation' : 'search-result'}
                  userId={msg.user_id}
                  content={msg.content}
                  createdAt={msg.created_at}
                  context={
                    channelNames.has(msg.channel_id)
                      ? `#${channelNames.get(msg.channel_id)}`
                      : conversation
                        ? 'Direct message'
                        : undefined
                  }
                  onClick={
                    conversation
                      ? onNavigateToConversation
                        ? () => onNavigateToConversation(msg.channel_id)
                        : undefined
                      : onNavigateToMessage
                        ? () => onNavigateToMessage(msg.channel_id, msg.id)
                        : undefined
                  }
                />
              );
            })}
          </div>
        ) : (
          <div className="px-4 py-8 text-center text-sm text-muted space-y-1">
            <p>Search across all messages in this workspace</p>
            <p className="text-xs text-subtle">
              Narrow it with the filters, or type <code>from:@name</code>, <code>in:#channel</code>,{' '}
              <code>has:file</code> or <code>after:2026-10-01</code>
            </p>
          </div>
        )}
      </div>
    </div>
  );
}
