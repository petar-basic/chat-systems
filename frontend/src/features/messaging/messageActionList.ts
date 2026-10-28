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

export function sheetActionsFor(message: Message, a: MessageActionHandlers): SheetAction[] {
  return [
    ...(a.onThread
      ? [{ key: 'thread', label: 'Reply in thread', Icon: MessageSquare, onSelect: a.onThread }]
      : []),
    ...(a.onToggleSave && a.onRemind
      ? [
          {
            key: 'save',
            label: a.saved ? 'Remove from Later' : 'Save for later',
            Icon: Bookmark,
            onSelect: a.onToggleSave,
          },
          {
            key: 'remind',
            label: 'Remind me in 1 hour',
            Icon: AlarmClock,
            onSelect: () => a.onRemind?.(new Date(Date.now() + 60 * 60 * 1000)),
          },
        ]
      : []),
    ...(a.onCopyLink ? [{ key: 'copy-link', label: 'Copy link', Icon: Link2, onSelect: a.onCopyLink }] : []),
    ...(a.onMarkUnread && !message.pending
      ? [{ key: 'mark-unread', label: 'Mark unread', Icon: MailOpen, onSelect: a.onMarkUnread }]
      : []),
    ...(a.onForward ? [{ key: 'forward', label: 'Forward', Icon: Forward, onSelect: a.onForward }] : []),
    ...(a.onTogglePin
      ? [
          {
            key: 'pin',
            label: message.is_pinned ? 'Unpin from channel' : 'Pin to channel',
            Icon: Pin,
            onSelect: a.onTogglePin,
          },
        ]
      : []),
    ...(a.canEdit ? [{ key: 'edit', label: 'Edit message', Icon: Pencil, onSelect: a.onEdit }] : []),
    ...(a.canDelete
      ? [{ key: 'delete', label: 'Delete message', Icon: Trash2, onSelect: a.onDelete, destructive: true }]
      : []),
  ];
}
