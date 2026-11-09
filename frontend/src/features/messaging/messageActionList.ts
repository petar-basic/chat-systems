import {
  AlarmClock,
  Bookmark,
  Forward,
  Link2,
  MailOpen,
  MessageSquare,
  Pencil,
  Pin,
  Trash2,
} from 'lucide-react';
import type { TFunction } from 'i18next';
import type { Message } from '@/stores/workspace';
import type { SheetAction } from './MessageActionSheet';

/// Reacting is the most common action in the product; a picker turns one tap
/// into three. Kept to three so the toolbar stays narrow enough not to cover
/// the message it belongs to.
export const QUICK_REACTIONS = ['👍', '✅', '🎉'];

export interface MessageActionHandlers {
  saved: boolean;
  canDelete: boolean;
  canEdit: boolean;
  canReact: boolean;
  onThread?: () => void;
  onToggleSave?: () => void;
  onRemind?: (at: Date) => void;
  onCopyLink?: () => void;
  onMarkUnread?: () => void;
  onForward?: () => void;
  onTogglePin?: () => void;
  onEdit: () => void;
  onDelete: () => void;
}

export function sheetActionsFor(
  message: Message,
  a: MessageActionHandlers,
  t: TFunction<'messages'>,
): SheetAction[] {
  return [
    ...(a.onThread
      ? [{ key: 'thread', label: t('actions.thread'), Icon: MessageSquare, onSelect: a.onThread }]
      : []),
    ...(a.onToggleSave && a.onRemind
      ? [
          {
            key: 'save',
            label: a.saved ? t('actions.unsave') : t('actions.save'),
            Icon: Bookmark,
            onSelect: a.onToggleSave,
          },
          {
            key: 'remind',
            label: t('sheet.remindInHour'),
            Icon: AlarmClock,
            onSelect: () => a.onRemind?.(new Date(Date.now() + 60 * 60 * 1000)),
          },
        ]
      : []),
    ...(a.onCopyLink
      ? [{ key: 'copy-link', label: t('actions.copyLink'), Icon: Link2, onSelect: a.onCopyLink }]
      : []),
    ...(a.onMarkUnread && !message.pending
      ? [{ key: 'mark-unread', label: t('actions.markUnread'), Icon: MailOpen, onSelect: a.onMarkUnread }]
      : []),
    ...(a.onForward
      ? [{ key: 'forward', label: t('actions.forward'), Icon: Forward, onSelect: a.onForward }]
      : []),
    ...(a.onTogglePin
      ? [
          {
            key: 'pin',
            label: message.is_pinned ? t('sheet.unpin') : t('sheet.pin'),
            Icon: Pin,
            onSelect: a.onTogglePin,
          },
        ]
      : []),
    ...(a.canEdit ? [{ key: 'edit', label: t('sheet.edit'), Icon: Pencil, onSelect: a.onEdit }] : []),
    ...(a.canDelete
      ? [{ key: 'delete', label: t('sheet.delete'), Icon: Trash2, onSelect: a.onDelete, destructive: true }]
      : []),
  ];
}
