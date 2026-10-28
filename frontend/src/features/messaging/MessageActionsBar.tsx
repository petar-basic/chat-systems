import { useRef, useState } from 'react';
import {
  Bookmark,
  Forward,
  Link2,
  MailOpen,
  MessageSquare,
  MoreHorizontal,
  Pencil,
  Pin,
  SmilePlus,
  Trash2,
} from 'lucide-react';
import { AnchoredPopover } from '@/shared/components/Popover/AnchoredPopover';
import type { Message } from '@/stores/workspace';
import EmojiPicker from './EmojiPicker';
import { REMIND_PRESETS } from './schedulePresets';
import { QUICK_REACTIONS, type MessageActionHandlers } from './messageActionList';

interface Props {
  message: Message;
  actions: MessageActionHandlers;
  pickerOpen: boolean;
  onPickerChange: (open: boolean) => void;
  onReact: (emoji: string) => void;
}

export function MessageActionsBar({ message, actions: a, pickerOpen, onPickerChange, onReact }: Props) {
  const [showOverflow, setShowOverflow] = useState(false);
  const overflowAnchorRef = useRef<HTMLButtonElement>(null);
  const reactBtnRef = useRef<HTMLButtonElement>(null);

  return (
    <div
      className={`message-actions absolute -top-3 right-2 flex items-center gap-0.5 bg-surface border border-line rounded-lg px-1 py-0.5 shadow-lg opacity-0 pointer-events-none transition-opacity group-hover:opacity-100 group-hover:pointer-events-auto group-focus-within:opacity-100 group-focus-within:pointer-events-auto ${
        showOverflow ? 'opacity-100 pointer-events-auto' : ''
      }`}
    >
      {a.canReact && (
        <>
          {/* Slack surfaces a few emoji inline because reacting is the most
            common action in the product, and a picker turns one tap into
            three. These are the three this instance uses most. */}
          {QUICK_REACTIONS.map((emoji) => (
            <button
              key={emoji}
              onClick={() => onReact(emoji)}
              aria-label={`React with ${emoji}`}
              data-qa="message-quick-reaction"
              className="px-1 py-0.5 text-sm leading-none hover:bg-raised rounded transition"
            >
              {emoji}
            </button>
          ))}
          <div className="w-px h-4 bg-raised mx-0.5" />
        </>
      )}
      {a.onThread && (
        <button
          onClick={a.onThread}
          aria-label="Reply in thread"
          data-qa="message-action-thread"
          className="p-1 text-muted hover:text-fg hover:bg-raised rounded transition"
        >
          <MessageSquare className="w-3.5 h-3.5" />
        </button>
      )}
      {a.canReact && (
        <div className="relative">
          <button
            ref={reactBtnRef}
            onClick={() => onPickerChange(!pickerOpen)}
            aria-label="Add reaction"
            data-qa="message-action-react"
            className="p-1 text-muted hover:text-fg hover:bg-raised rounded transition"
          >
            <SmilePlus className="w-3.5 h-3.5" />
          </button>
          {pickerOpen && (
            <EmojiPicker
              anchorRef={reactBtnRef}
              onSelect={(emoji) => {
                onReact(emoji);
                onPickerChange(false);
              }}
              onClose={() => onPickerChange(false)}
            />
          )}
        </div>
      )}
      {a.onToggleSave && (
        <button
          onClick={a.onToggleSave}
          aria-label={a.saved ? 'Remove from Later' : 'Save for later'}
          aria-pressed={a.saved}
          data-qa="message-action-save"
          className={`p-1 hover:bg-raised rounded transition ${a.saved ? 'text-accent' : 'text-muted hover:text-accent-soft'}`}
        >
          <Bookmark className="w-3.5 h-3.5" fill={a.saved ? 'currentColor' : 'none'} />
        </button>
      )}

      {/* The toolbar sits over the message, so it has to stay short. The
        four that get used live here; the rest are one click further. */}
      <div className="relative">
        <button
          ref={overflowAnchorRef}
          onClick={() => setShowOverflow((open) => !open)}
          aria-label="More actions"
          aria-expanded={showOverflow}
          data-qa="message-action-more"
          className="p-1 text-muted hover:text-fg hover:bg-raised rounded transition"
        >
          <MoreHorizontal className="w-3.5 h-3.5" />
        </button>
        {showOverflow && (
          <AnchoredPopover
            anchorRef={overflowAnchorRef}
            onClose={() => setShowOverflow(false)}
            dataQa="message-overflow"
            className="w-44 bg-surface border border-line rounded-lg shadow-xl py-1"
          >
            {a.onTogglePin && (
              <button
                onClick={() => {
                  a.onTogglePin?.();
                  setShowOverflow(false);
                }}
                data-qa="message-action-pin"
                className="w-full px-3 py-1.5 text-left text-sm text-fg-dim hover:bg-raised flex items-center gap-2"
              >
                <Pin className="w-3.5 h-3.5" /> {message.is_pinned ? 'Unpin' : 'Pin'}
              </button>
            )}
            {a.onMarkUnread && !message.pending && (
              <button
                onClick={() => {
                  a.onMarkUnread?.();
                  setShowOverflow(false);
                }}
                data-qa="message-action-mark-unread"
                className="w-full px-3 py-1.5 text-left text-sm text-fg-dim hover:bg-raised flex items-center gap-2"
              >
                <MailOpen className="w-3.5 h-3.5" /> Mark unread
              </button>
            )}
            {a.onCopyLink && (
              <button
                onClick={() => {
                  a.onCopyLink?.();
                  setShowOverflow(false);
                }}
                data-qa="message-action-copy-link"
                className="w-full px-3 py-1.5 text-left text-sm text-fg-dim hover:bg-raised flex items-center gap-2"
              >
                <Link2 className="w-3.5 h-3.5" /> Copy link
              </button>
            )}
            {a.onRemind && (
              <div className="border-y border-line/60 py-1" data-qa="message-action-remind">
                <div className="px-3 py-1 text-[11px] uppercase tracking-wide text-subtle">
                  Remind me about this
                </div>
                {REMIND_PRESETS.map((preset) => (
                  <button
                    key={preset.label}
                    onClick={() => {
                      a.onRemind?.(preset.at());
                      setShowOverflow(false);
                    }}
                    className="w-full px-3 py-1.5 text-left text-sm text-fg-dim hover:bg-raised"
                  >
                    {preset.label}
                  </button>
                ))}
              </div>
            )}
            {a.onForward && (
              <button
                onClick={() => {
                  a.onForward?.();
                  setShowOverflow(false);
                }}
                data-qa="message-action-forward"
                className="w-full px-3 py-1.5 text-left text-sm text-fg-dim hover:bg-raised flex items-center gap-2"
              >
                <Forward className="w-3.5 h-3.5" /> Forward
              </button>
            )}
            {a.canEdit && (
              <button
                onClick={() => {
                  a.onEdit();
                  setShowOverflow(false);
                }}
                data-qa="message-action-edit"
                className="w-full px-3 py-1.5 text-left text-sm text-fg-dim hover:bg-raised flex items-center gap-2"
              >
                <Pencil className="w-3.5 h-3.5" /> Edit
              </button>
            )}
            {a.canDelete && (
              <button
                onClick={() => {
                  a.onDelete();
                  setShowOverflow(false);
                }}
                data-qa="message-action-delete"
                className="w-full px-3 py-1.5 text-left text-sm text-danger hover:bg-raised flex items-center gap-2"
              >
                <Trash2 className="w-3.5 h-3.5" /> Delete
              </button>
            )}
          </AnchoredPopover>
        )}
      </div>
    </div>
  );
}
