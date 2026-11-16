import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router';
import { Hash, MessageSquare, Paperclip, Trash2 } from 'lucide-react';
import type { Channel } from '@/stores/workspace';
import { useWorkspaceStore } from '@/stores/workspace';
import type { Conversation } from '@/features/messaging';
import { ownDrafts, useDraftStore, type LocalDraft } from '@/stores/drafts';
import { useUserCache } from '@/stores/users';
import { IconButton } from '@/shared/components/IconButton/IconButton';
import { ROUTES } from '@/shared/constants';
import { formatDateTime } from '@/lib/datetime';
import { toPlainText } from '@/lib/plainText';
import { targetLabel } from '@/lib/targetLabel';

interface Props {
  workspaceId: string;
  channels: Channel[];
  conversations: Conversation[];
  onOpenThread: (rootId: string) => void;
  onOpened: () => void;
}

export function DraftsList({ workspaceId, channels, conversations, onOpenThread, onOpened }: Props) {
  const { t } = useTranslation('messages');
  const navigate = useNavigate();
  const drafts = useDraftStore((s) => s.drafts);
  const currentUserId = useWorkspaceStore((s) => s.currentUserId);
  const { getUser } = useUserCache();

  const written = ownDrafts(drafts)
    .filter(([, draft]) => draft.text && draft.place?.workspaceId === workspaceId)
    .sort(([, a], [, b]) => Date.parse(b.at) - Date.parse(a.at));

  const open = (draft: LocalDraft) => {
    const place = draft.place;
    if (!place) return;
    if (place.threadParentId) {
      onOpenThread(place.threadParentId);
      return;
    }
    const isConversation = conversations.some((c) => c.id === place.channelId);
    navigate(
      isConversation
        ? ROUTES.conversation(workspaceId, place.channelId)
        : ROUTES.channel(workspaceId, place.channelId),
    );
    onOpened();
  };

  const targetOf = (draft: LocalDraft) => {
    const channelId = draft.place?.channelId ?? '';
    const label = targetLabel(
      channelId,
      channels,
      conversations,
      currentUserId ?? undefined,
      (id) => getUser(id)?.display_name,
      t('scheduled.leftChannel'),
    );
    return draft.place?.threadParentId ? t('scheduled.thread', { target: label }) : label;
  };

  if (written.length === 0) {
    return (
      <div className="text-center py-10 px-6 text-sm text-muted" data-qa="drafts-empty">
        {t('drafts.empty')}
      </div>
    );
  }

  return (
    <ul className="flex-1 overflow-y-auto" data-qa="drafts-list">
      {written.map(([key, draft]) => (
        <li key={key} className="relative border-b border-line/40" data-qa="draft-row">
          <button
            type="button"
            onClick={() => open(draft)}
            className="w-full text-left px-4 py-3 pr-12 hover:bg-raised/30 transition"
            data-qa="draft-open"
          >
            <span className="text-xs text-muted flex items-center gap-1.5" data-qa="draft-target">
              {conversations.some((c) => c.id === draft.place?.channelId) ? (
                <MessageSquare className="w-3 h-3 shrink-0" />
              ) : (
                <Hash className="w-3 h-3 shrink-0" />
              )}
              <span className="truncate">{targetOf(draft)}</span>
              <span className="ml-auto shrink-0">{formatDateTime(draft.at)}</span>
            </span>
            <span className="block text-sm text-fg-soft mt-0.5 line-clamp-2" data-qa="draft-content">
              {toPlainText(draft.text)}
            </span>
            {draft.filesLeftBehind && (
              <span className="text-xs text-muted mt-0.5 flex items-center gap-1" data-qa="draft-files-left">
                <Paperclip className="w-3 h-3 shrink-0" />
                {t('drafts.filesLeftBehind')}
              </span>
            )}
          </button>
          <IconButton
            label={t('drafts.delete')}
            icon={<Trash2 className="h-4 w-4" />}
            tone="danger"
            onClick={() => useDraftStore.getState().discardDraft(key)}
            className="absolute right-3 top-3"
            data-qa="draft-delete"
          />
        </li>
      ))}
    </ul>
  );
}
