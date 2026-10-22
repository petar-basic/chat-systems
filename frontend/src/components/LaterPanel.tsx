import { useState } from 'react';
import { Archive, ArchiveRestore, Bookmark, Check, Hash, MessageSquare, Trash2 } from 'lucide-react';
import { PanelHeader } from '@/shared/components/PanelHeader/PanelHeader';
import { Spinner } from '@/shared/components/Spinner/Spinner';
import { IconButton } from '@/shared/components/IconButton/IconButton';
import type { Channel } from '@/stores/workspace';
import type { Conversation } from '@/hooks/queries/useConversations';
import { useLater, useLaterActions, type LaterItem, type LaterStatus } from '@/hooks/queries/useLater';
import { RemindMenu } from '@/features/later/RemindMenu';
import { formatScheduleHint } from '@/features/messaging/schedulePresets';
import { targetLabel } from '@/lib/targetLabel';
import { displayNameOf } from '@/lib/userHelpers';
import { useUserCache } from '@/stores/users';
import { useWorkspaceStore } from '@/stores/workspace';
import RichTextDisplay from './RichTextDisplay';

const TABS: { status: LaterStatus; label: string }[] = [
  { status: 'in_progress', label: 'In progress' },
  { status: 'archived', label: 'Archived' },
  { status: 'completed', label: 'Completed' },
];

const EMPTY: Record<LaterStatus, string> = {
  in_progress: 'Save a message or set a reminder and it waits here until you tick it off.',
  archived: 'Nothing archived.',
  completed: 'Nothing completed yet.',
};

interface Props {
  workspaceId: string;
  instanceUrl?: string;
  channels: Channel[];
  conversations: Conversation[];
  onClose: () => void;
  onNavigateToMessage: (channelId: string, messageId: string) => void;
  onOpenConversation: (conversationId: string) => void;
}

export default function LaterPanel({
  workspaceId,
  instanceUrl,
  channels,
  conversations,
  onClose,
  onNavigateToMessage,
  onOpenConversation,
}: Props) {
  const [status, setStatus] = useState<LaterStatus>('in_progress');
  const { data: items = [], isLoading } = useLater(workspaceId, status, instanceUrl);
  const { data: open = [] } = useLater(workspaceId, 'in_progress', instanceUrl);
  const actions = useLaterActions(workspaceId, instanceUrl);
  const [text, setText] = useState('');

  const openItem = (item: LaterItem) => {
    if (!item.message) return;
    if (conversations.some((c) => c.id === item.message?.channel_id))
      onOpenConversation(item.message.channel_id);
    else onNavigateToMessage(item.message.channel_id, item.message.id);
  };

  return (
    <div
      className="w-full lg:w-80 max-lg:fixed max-lg:inset-0 max-lg:z-40 bg-surface border-l border-line/50 flex flex-col"
      data-qa="later-panel"
    >
      <PanelHeader title="Later" icon={<Bookmark className="h-4 w-4 text-muted" />} onClose={onClose} />

      <div className="flex gap-1 border-b border-line/50 px-3 py-2" role="tablist">
        {TABS.map((tab) => (
          <button
            key={tab.status}
            role="tab"
            aria-selected={status === tab.status}
            data-qa={`later-tab-${tab.status}`}
            onClick={() => setStatus(tab.status)}
            className={`rounded-full px-2.5 py-1 text-xs transition ${
              status === tab.status ? 'bg-purple-600 text-white' : 'text-fg-dim hover:bg-raised'
            }`}
          >
            {tab.label}
            {tab.status === 'in_progress' && open.length > 0 && ` ${open.length}`}
          </button>
        ))}
      </div>

      {status === 'in_progress' && (
        <div className="flex items-center gap-1.5 border-b border-line/40 px-3 py-2">
          <input
            value={text}
            onChange={(e) => setText(e.target.value)}
            placeholder="Remind me to…"
            aria-label="New reminder"
            data-qa="later-new-text"
            className="min-w-0 flex-1 px-2 py-1.5 bg-raised/50 border border-line-strong rounded text-sm text-fg placeholder-muted focus:outline-none focus:ring-2 focus:ring-purple-500"
          />
          {text.trim() && (
            <RemindMenu
              label="Pick when"
              dataQa="later-new-when"
              onPick={(at) =>
                actions.remind.mutate({ text: text.trim(), remindAt: at }, { onSuccess: () => setText('') })
              }
            />
          )}
        </div>
      )}

      <div className="flex-1 overflow-y-auto">
        {isLoading ? (
          <div className="flex justify-center py-8 text-accent">
            <Spinner />
          </div>
        ) : items.length === 0 ? (
          <p className="px-6 py-10 text-center text-sm text-muted" data-qa="later-empty">
            {EMPTY[status]}
          </p>
        ) : (
          items.map((item) => (
            <LaterRow
              key={item.id}
              item={item}
              channels={channels}
              conversations={conversations}
              onOpen={() => openItem(item)}
              onComplete={() =>
                actions.update.mutate({
                  id: item.id,
                  status: item.status === 'completed' ? 'in_progress' : 'completed',
                })
              }
              onArchive={() =>
                actions.update.mutate({
                  id: item.id,
                  status: item.status === 'archived' ? 'in_progress' : 'archived',
                })
              }
              onRemind={(at) => actions.update.mutate({ id: item.id, remindAt: at })}
              onClearReminder={() => actions.update.mutate({ id: item.id, clearReminder: true })}
              onRemove={() => actions.remove.mutate(item.id)}
            />
          ))
        )}
      </div>
    </div>
  );
}

function dueChip(item: LaterItem) {
  if (!item.remind_at || item.status !== 'in_progress') return null;
  const at = new Date(item.remind_at);
  const overdue = at.getTime() <= Date.now();
  return (
    <span
      data-qa="later-due"
      data-overdue={overdue || undefined}
      className={`rounded px-1.5 py-0.5 text-[11px] font-medium ${
        overdue ? 'bg-red-500/15 text-danger' : 'bg-raised text-fg-dim'
      }`}
    >
      {overdue ? 'Overdue · ' : 'Due '}
      {formatScheduleHint(at)}
    </span>
  );
}

interface RowProps {
  item: LaterItem;
  channels: Channel[];
  conversations: Conversation[];
  onOpen: () => void;
  onComplete: () => void;
  onArchive: () => void;
  onRemind: (at: Date) => void;
  onClearReminder: () => void;
  onRemove: () => void;
}

function LaterRow({
  item,
  channels,
  conversations,
  onOpen,
  onComplete,
  onArchive,
  onRemind,
  onClearReminder,
  onRemove,
}: RowProps) {
  const { getUser } = useUserCache();
  const currentUserId = useWorkspaceStore((s) => s.currentUserId);
  const message = item.message;
  const channelId = message?.channel_id ?? item.channel_id;
  const where = channelId
    ? targetLabel(
        channelId,
        channels,
        conversations,
        currentUserId ?? undefined,
        (id) => getUser(id)?.display_name,
      )
    : null;
  const isConversation = !!channelId && conversations.some((c) => c.id === channelId);
  const completed = item.status === 'completed';
  const from =
    item.created_by !== currentUserId ? displayNameOf(getUser(item.created_by)?.display_name) : null;

  return (
    <div
      className="group flex gap-2.5 border-b border-line/50 px-3 py-2.5"
      data-qa="later-row"
      data-status={item.status}
    >
      <button
        type="button"
        role="checkbox"
        aria-checked={completed}
        aria-label={completed ? 'Mark as not done' : 'Mark as done'}
        data-qa="later-complete"
        onClick={onComplete}
        className={`mt-0.5 flex h-4 w-4 shrink-0 items-center justify-center rounded border transition ${
          completed
            ? 'border-purple-500 bg-purple-600 text-white'
            : 'border-line-strong hover:border-purple-500'
        }`}
      >
        {completed && <Check className="h-3 w-3" />}
      </button>

      <div className="min-w-0 flex-1">
        <div className="mb-0.5 flex flex-wrap items-center gap-1.5 text-xs text-muted">
          {where && (
            <span className="flex min-w-0 items-center gap-1">
              {isConversation ? (
                <MessageSquare className="h-3 w-3 shrink-0" />
              ) : (
                <Hash className="h-3 w-3 shrink-0" />
              )}
              <span className="truncate">{where}</span>
            </span>
          )}
          {message && <span>· {displayNameOf(getUser(message.author_id)?.display_name)}</span>}
          {from && <span>· from {from}</span>}
          {dueChip(item)}
        </div>
        <button
          type="button"
          onClick={onOpen}
          disabled={!message}
          className={`block w-full text-left text-sm text-fg-soft line-clamp-3 disabled:cursor-default ${completed ? 'line-through opacity-70' : ''}`}
          data-qa="later-content"
        >
          {message ? (
            <RichTextDisplay content={message.content} attachments={message.attachments} compact />
          ) : (
            item.text
          )}
        </button>
        {message && item.text && <p className="mt-0.5 text-xs text-fg-dim">{item.text}</p>}
        {item.note && <p className="mt-0.5 text-xs text-accent-soft">{item.note}</p>}
      </div>

      <div className="-mr-1 flex shrink-0 items-start gap-0.5 opacity-0 transition group-hover:opacity-100 focus-within:opacity-100">
        {item.status === 'in_progress' && (
          <RemindMenu
            onPick={onRemind}
            onClear={item.remind_at ? onClearReminder : undefined}
            dataQa="later-remind"
          />
        )}
        <IconButton
          label={item.status === 'archived' ? 'Move back to in progress' : 'Archive'}
          icon={
            item.status === 'archived' ? (
              <ArchiveRestore className="h-4 w-4" />
            ) : (
              <Archive className="h-4 w-4" />
            )
          }
          onClick={onArchive}
          data-qa="later-archive"
        />
        <IconButton
          label="Remove"
          icon={<Trash2 className="h-4 w-4" />}
          tone="danger"
          onClick={onRemove}
          data-qa="later-remove"
        />
      </div>
    </div>
  );
}
