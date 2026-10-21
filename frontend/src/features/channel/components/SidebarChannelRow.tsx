import { useRef, useState } from 'react';
import { Bell, BellOff, Hash, Lock, FolderInput } from 'lucide-react';
import { AnchoredPopover } from '@/shared/components/Popover/AnchoredPopover';
import { CHANNEL_DRAG_TYPE } from './channelDrag';
import type { Channel } from '@/stores/workspace';

/** Slack stops at 99+, and so does the space in the sidebar. */
function formatBadge(count: number): string {
  return count > 99 ? '99+' : String(count);
}

interface Props {
  channel: Channel;
  active: boolean;
  muted: boolean;
  unread: boolean;
  mentioned: boolean;
  unreadCount: number | undefined;
  mentionCount: number | undefined;
  onSelect: (channel: Channel) => void;
  onToggleMute: (channelId: string, muted: boolean) => void;
  moveTargets?: { id: string | null; name: string }[];
  sectionId?: string | null;
  onMove?: (channelId: string, sectionId: string | null) => void;
}

export function SidebarChannelRow({
  channel,
  active,
  muted,
  unread,
  mentioned,
  unreadCount,
  mentionCount,
  onSelect,
  onToggleMute,
  moveTargets = [],
  sectionId = null,
  onMove,
}: Props) {
  const [moveOpen, setMoveOpen] = useState(false);
  const moveAnchor = useRef<HTMLButtonElement>(null);
  const targets = moveTargets.filter((target) => target.id !== sectionId);
  const icon =
    channel.channel_type === 'private' ? (
      <Lock className="w-4 h-4 text-muted shrink-0" />
    ) : (
      <Hash className="w-4 h-4 text-muted shrink-0" />
    );
  const showUnread = unread && !muted;
  return (
    <div
      className="group relative flex items-center"
      draggable={!!onMove}
      onDragStart={(event) => {
        event.dataTransfer.setData(CHANNEL_DRAG_TYPE, channel.id);
        event.dataTransfer.effectAllowed = 'move';
      }}
      data-qa="sidebar-channel-row"
    >
      <button
        onClick={() => onSelect(channel)}
        className={`flex-1 min-w-0 px-3 py-1.5 flex items-center gap-2 text-sm transition ${
          active
            ? 'bg-purple-600/20 text-fg'
            : showUnread
              ? 'text-fg font-semibold hover:bg-raised/30'
              : muted
                ? 'text-subtle hover:bg-raised/30'
                : 'text-muted hover:bg-raised/30 hover:text-fg-soft'
        }`}
      >
        {icon}
        <span className="truncate">{channel.name || 'Channel'}</span>
        {muted && <BellOff className="w-3 h-3 text-faint ml-auto shrink-0" />}
        {!muted && mentioned ? (
          <span
            aria-hidden="true"
            data-qa="channel-mention-badge"
            data-channel-id={channel.id}
            className="ml-auto min-w-5 h-5 px-1 bg-red-500 rounded-full shrink-0 flex items-center justify-center text-[10px] font-bold text-white"
          >
            {mentionCount ? formatBadge(mentionCount) : '@'}
          </span>
        ) : (
          showUnread &&
          !!unreadCount && (
            <span
              aria-hidden="true"
              data-qa="channel-unread-badge"
              data-channel-id={channel.id}
              className="ml-auto min-w-5 h-5 px-1 bg-elevated rounded-full shrink-0 flex items-center justify-center text-[10px] font-bold text-fg"
            >
              {formatBadge(unreadCount)}
            </span>
          )
        )}
      </button>
      {onMove && targets.length > 0 && (
        <button
          ref={moveAnchor}
          onClick={() => setMoveOpen((open) => !open)}
          aria-label={`Move ${channel.name} to a section`}
          aria-expanded={moveOpen}
          title="Move to…"
          data-qa="sidebar-channel-move"
          className="absolute right-7 hidden group-hover:flex focus-visible:flex p-1 rounded text-muted hover:text-fg hover:bg-raised"
        >
          <FolderInput className="w-3 h-3" />
        </button>
      )}
      {moveOpen && onMove && (
        <AnchoredPopover
          anchorRef={moveAnchor}
          onClose={() => setMoveOpen(false)}
          className="w-48 bg-surface border border-line rounded-lg shadow-xl py-1"
        >
          <div className="px-3 py-1 text-xs text-muted">Move to</div>
          {targets.map((target) => (
            <button
              key={target.id ?? 'default'}
              type="button"
              onClick={() => {
                setMoveOpen(false);
                onMove(channel.id, target.id);
              }}
              className="w-full px-3 py-1.5 text-left text-sm text-fg-dim hover:bg-raised truncate"
            >
              {target.name}
            </button>
          ))}
        </AnchoredPopover>
      )}
      <button
        onClick={() => onToggleMute(channel.id, !muted)}
        aria-label={muted ? `Unmute ${channel.name}` : `Mute ${channel.name}`}
        title={muted ? 'Unmute' : 'Mute'}
        className="absolute right-1 hidden group-hover:flex p-1 rounded text-muted hover:text-fg hover:bg-raised"
      >
        {muted ? <Bell className="w-3 h-3" /> : <BellOff className="w-3 h-3" />}
      </button>
    </div>
  );
}
