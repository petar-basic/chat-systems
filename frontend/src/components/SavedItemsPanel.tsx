import { X, Bookmark, Hash, MessageSquare, Trash2 } from 'lucide-react';
import type { Channel } from '@/stores/workspace';
import type { Conversation } from '@/hooks/queries/useConversations';
import { useSavedItems, useUnsaveMessage, type SavedItem } from '@/hooks/queries/useSaved';
import { conversationTitle } from '@/lib/conversationHelpers';
import { displayNameOf } from '@/lib/userHelpers';
import { useUserCache } from '@/stores/users';
import { useWorkspaceStore } from '@/stores/workspace';
import RichTextDisplay from './RichTextDisplay';

interface Props {
  workspaceId: string;
  instanceUrl?: string;
  channels: Channel[];
  conversations: Conversation[];
  onClose: () => void;
  onNavigateToMessage: (channelId: string, messageId: string) => void;
  onOpenConversation: (conversationId: string) => void;
}

export default function SavedItemsPanel({
  workspaceId,
  instanceUrl,
  channels,
  conversations,
  onClose,
  onNavigateToMessage,
  onOpenConversation,
}: Props) {
  const { data: items = [], isLoading } = useSavedItems(workspaceId, instanceUrl);
  const unsave = useUnsaveMessage(workspaceId, instanceUrl);
  const { getUser } = useUserCache();
  const currentUserId = useWorkspaceStore((s) => s.currentUserId);

  const targetOf = (item: SavedItem) => {
    if (item.channel_id) {
      const channel = channels.find((c) => c.id === item.channel_id);
      return channel ? `#${channel.name}` : 'a channel you left';
    }
    const conversation = conversations.find((c) => c.id === item.conversation_id);
    return conversation
      ? conversationTitle(conversation, currentUserId ?? undefined, (id) => getUser(id)?.display_name)
      : 'a conversation you left';
  };

  const open = (item: SavedItem) => {
    if (item.channel_id && item.message_id) onNavigateToMessage(item.channel_id, item.message_id);
    else if (item.conversation_id) onOpenConversation(item.conversation_id);
  };

  return (
    <div
      className="w-full lg:w-80 max-lg:fixed max-lg:inset-0 max-lg:z-40 bg-surface border-l border-line/50 flex flex-col"
      data-qa="saved-panel"
    >
      <div className="h-14 px-4 flex items-center justify-between border-b border-line/50 shrink-0">
        <div className="flex items-center gap-2">
          <Bookmark className="w-4 h-4 text-muted" />
          <span className="font-semibold">Saved</span>
        </div>
        <button
          onClick={onClose}
          aria-label="Close saved items"
          className="text-muted hover:text-fg transition cursor-pointer"
        >
          <X className="w-4 h-4" />
        </button>
      </div>

      <div className="flex-1 overflow-y-auto">
        {isLoading ? (
          <div className="flex justify-center py-8">
            <div className="w-5 h-5 border-2 border-purple-500/30 border-t-purple-500 rounded-full animate-spin" />
          </div>
        ) : items.length === 0 ? (
          <div className="text-center py-10 px-6 text-sm text-muted" data-qa="saved-empty">
            Nothing saved yet. Hover a message and pick the bookmark to keep it here.
          </div>
        ) : (
          items.map((item) => {
            const author = getUser(item.author_id);
            return (
              <div
                key={item.id}
                className="px-4 py-3 border-b border-line/40"
                data-qa="saved-row"
                data-saved-id={item.id}
              >
                <div className="flex items-start gap-2">
                  <button
                    onClick={() => open(item)}
                    className="flex-1 min-w-0 text-left cursor-pointer"
                    data-qa="saved-open"
                  >
                    <div className="text-xs text-muted flex items-center gap-1.5">
                      {item.channel_id ? (
                        <Hash className="w-3 h-3 shrink-0" />
                      ) : (
                        <MessageSquare className="w-3 h-3 shrink-0" />
                      )}
                      <span className="truncate">{targetOf(item)}</span>
                      <span className="text-subtle">·</span>
                      <span className="truncate">{displayNameOf(author?.display_name)}</span>
                    </div>
                    <div className="text-sm text-fg-soft mt-0.5 line-clamp-3" data-qa="saved-content">
                      <RichTextDisplay content={item.content} />
                    </div>
                    {item.note && (
                      <div className="text-xs text-accent-soft mt-1" data-qa="saved-note">
                        {item.note}
                      </div>
                    )}
                  </button>
                  <button
                    onClick={() => unsave.mutate(item.id)}
                    disabled={unsave.isPending}
                    aria-label="Remove from saved"
                    title="Remove from saved"
                    data-qa="saved-remove"
                    className="p-1.5 rounded text-muted hover:text-danger hover:bg-raised transition cursor-pointer disabled:opacity-50"
                  >
                    <Trash2 className="w-4 h-4" />
                  </button>
                </div>
              </div>
            );
          })
        )}
      </div>
    </div>
  );
}
