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
  forTab,
  hasFilters,
  searchParams,
  type FilterContext,
  type SearchFilters,
  type SearchTab,
} from '@/features/search/searchFilters';
import { FileHit, type FileSearchHit } from '@/features/search/FileHit';
import { isRenderableImage } from '@/features/messaging/fileKinds';
import { ImageGallery } from '@/features/messaging/ImageGallery';
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

const TABS: { tab: SearchTab; label: string }[] = [
  { tab: 'messages', label: 'Messages' },
  { tab: 'files', label: 'Files' },
];

const LIMIT = 20;

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
  const [tab, setTab] = useState<SearchTab>('messages');
  const [results, setResults] = useState<Message[]>([]);
  const [files, setFiles] = useState<FileSearchHit[]>([]);
  const [viewing, setViewing] = useState<number | null>(null);
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
    async (q: string, all: SearchFilters, on: SearchTab) => {
      const request = ++requestRef.current;
      const active = forTab(all, on);
      if ((!q.trim() && !hasFilters(active)) || !workspaceId) {
        setResults([]);
        setFiles([]);
        setSearched(false);
        setError(null);
        setLoading(false);
        return;
      }
      setLoading(true);
      setSearched(true);
      setError(null);
      const params = { q: q.trim(), workspace_id: workspaceId, limit: LIMIT, ...searchParams(active) };
      const api = getApiForInstance(instanceUrl);
      try {
        if (on === 'files') {
          const res = await api.typed((c) => c.GET('/search/files', { params: { query: params } }));
          if (request === requestRef.current) setFiles(res.data);
        } else {
          const res = await api.typed((c) => c.GET('/search', { params: { query: params } }));
          if (request === requestRef.current) setResults(res.data);
        }
      } catch (err: unknown) {
        if (request !== requestRef.current) return;
        setResults([]);
        setFiles([]);
        setError(toUserMessage(err, 'Search failed'));
      } finally {
        if (request === requestRef.current) setLoading(false);
      }
    },
    [workspaceId, instanceUrl],
  );

  const search = (text: string, active: SearchFilters, delay: number, on: SearchTab = tab) => {
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => void doSearch(text, active, on), delay);
  };

  const apply = (text: string, final: boolean) => {
    const extracted = extractFilters(text, context, final);
    const nextText = extracted?.text ?? text;
    const nextFilters = extracted ? { ...filters, ...extracted.filters } : filters;
    const nextTab = extracted?.filters.kind ? 'files' : tab;
    setQuery(nextText);
    if (extracted) setFilters(nextFilters);
    setTab(nextTab);
    search(nextText, nextFilters, final || extracted ? 0 : 400, nextTab);
  };

  const changeFilters = (next: SearchFilters) => {
    setFilters(next);
    search(query, next, 0);
  };

  const changeTab = (next: SearchTab) => {
    setTab(next);
    search(query, filters, 0, next);
  };

  const filtered = hasFilters(forTab(filters, tab));
  const noun = tab === 'files' ? 'files' : 'messages';
  const count = tab === 'files' ? files.length : results.length;
  const images = files.filter((file) => isRenderableImage(file.mime_type));
  const placeOf = (channelId: string) =>
    channelNames.has(channelId)
      ? `#${channelNames.get(channelId)}`
      : conversationIds.has(channelId)
        ? 'Direct message'
        : undefined;
  const openFile = (file: FileSearchHit) => {
    if (isRenderableImage(file.mime_type)) return () => setViewing(images.indexOf(file));
    if (conversationIds.has(file.channel_id)) {
      return onNavigateToConversation ? () => onNavigateToConversation(file.channel_id) : undefined;
    }
    return onNavigateToMessage ? () => onNavigateToMessage(file.channel_id, file.message_id) : undefined;
  };

  return (
    <div className="w-full lg:w-80 max-lg:fixed max-lg:inset-0 max-lg:z-40 flex flex-col border-l border-line/50 bg-app lg:bg-app/80">
      <PanelHeader title="Search" icon={<Search className="h-4 w-4 text-muted" />} onClose={onClose} />

      <div className="px-3 py-3 border-b border-line/30 space-y-2">
        <div className="flex gap-1" role="tablist">
          {TABS.map((t) => (
            <button
              key={t.tab}
              type="button"
              role="tab"
              aria-selected={tab === t.tab}
              data-qa={`search-tab-${t.tab}`}
              onClick={() => changeTab(t.tab)}
              className={`rounded-full px-2.5 py-1 text-xs transition ${
                tab === t.tab ? 'bg-purple-600 text-white' : 'text-fg-dim hover:bg-raised'
              }`}
            >
              {t.label}
            </button>
          ))}
        </div>
        <TextInput
          label={`Search ${noun}`}
          labelHidden
          ref={inputRef}
          value={query}
          onChange={(e) => apply(e.target.value, false)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') apply(query, true);
          }}
          placeholder={`Search ${noun}...`}
          title={`Narrow with from:@name in:#channel ${tab === 'files' ? 'type:pdf' : 'has:file'} before:2026-10-01 after: on:`}
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
        <SearchFilterBar filters={filters} onChange={changeFilters} context={context} tab={tab} />
      </div>

      {searched && !loading && !error && (
        <div
          className="flex items-center justify-between px-4 pt-2 text-xs text-muted"
          data-qa="search-summary"
        >
          <span>
            {count === LIMIT ? `${LIMIT}+` : count} {count === 1 ? 'result' : 'results'}
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
        ) : searched && count === 0 ? (
          <div className="text-center py-8 text-muted text-sm">
            {query.trim() ? (
              <>
                No {noun} found for &ldquo;{query.trim()}&rdquo;{filtered && ' with these filters'}
              </>
            ) : (
              `No ${noun} match these filters`
            )}
          </div>
        ) : tab === 'files' && files.length > 0 ? (
          <div className="space-y-1" data-qa="file-results">
            {files.map((file) => (
              <FileHit
                key={file.id}
                hit={file}
                query={query}
                context={placeOf(file.channel_id)}
                onOpen={openFile(file)}
              />
            ))}
          </div>
        ) : tab === 'messages' && results.length > 0 ? (
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
                  context={placeOf(msg.channel_id)}
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
            <p>
              {tab === 'files'
                ? 'Find a file by any part of its name'
                : 'Search across all messages in this workspace'}
            </p>
            <p className="text-xs text-subtle">
              Narrow it with the filters, or type <code>from:@name</code>, <code>in:#channel</code>,{' '}
              <code>{tab === 'files' ? 'type:pdf' : 'has:file'}</code> or <code>after:2026-10-01</code>
            </p>
          </div>
        )}
      </div>

      {viewing !== null && (
        <ImageGallery
          items={images.map((image) => ({
            attachment: image,
            userId: image.user_id,
            createdAt: image.created_at,
          }))}
          index={viewing}
          onIndexChange={setViewing}
          onClose={() => setViewing(null)}
        />
      )}
    </div>
  );
}
