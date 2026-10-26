import { useRef, useState, type KeyboardEvent, type RefObject } from 'react';
import { Bell, BellOff, BellRing, Check } from 'lucide-react';
import { AnchoredPopover } from '@/shared/components/Popover/AnchoredPopover';
import { IconButton } from '@/shared/components/IconButton/IconButton';
import { toast } from '@/shared/components/Toast';
import { useWorkspaceStore } from '@/stores/workspace';
import { useWorkspaceChannels } from '@/features/workspace';
import { useConversations } from '@/features/messaging';
import { useSetChannelMuted } from '@/features/channel';
import {
  useNotifyDefault,
  useSetNotifyLevel,
  type NotifyLevel,
} from '@/features/notifications/hooks/useNotifyLevels';
import { NOTIFY_LEVEL_LABELS } from './notifyLevels';

const LEVELS: NotifyLevel[] = ['all', 'mentions', 'nothing'];

interface Target {
  channelId: string;
  direct: boolean;
}

function useNotifySettings({ channelId, direct }: Target) {
  const workspace = useWorkspaceStore((s) => s.currentWorkspace);
  const workspaceId = workspace?.id ?? null;
  const instanceUrl = workspace?.instanceUrl;
  const { data: channels } = useWorkspaceChannels(direct ? null : workspaceId, instanceUrl);
  const { data: conversations } = useConversations(direct ? workspaceId : null, instanceUrl);
  const { data: workspaceDefault } = useNotifyDefault(workspaceId, instanceUrl);
  const muted = useWorkspaceStore((s) => s.mutedChannels.has(channelId));
  const entry = direct
    ? conversations?.find((c) => c.id === channelId)
    : channels?.find((c) => c.id === channelId);
  return {
    workspaceId: workspaceId ?? '',
    instanceUrl,
    stored: entry?.notify_level ?? null,
    effective: entry?.effective_notify_level ?? (direct ? 'all' : (workspaceDefault ?? 'mentions')),
    defaultLevel: direct ? 'all' : (workspaceDefault ?? 'mentions'),
    muted: !direct && muted,
  };
}

interface MenuProps extends Target {
  anchorRef: RefObject<HTMLElement | null>;
  onClose: () => void;
}

export function NotificationLevelMenu({ channelId, direct, anchorRef, onClose }: MenuProps) {
  const settings = useNotifySettings({ channelId, direct });
  const setLevel = useSetNotifyLevel(settings.workspaceId, settings.instanceUrl);
  const setMuted = useSetChannelMuted(settings.workspaceId, settings.instanceUrl);
  const menuRef = useRef<HTMLDivElement>(null);

  const choose = (level: NotifyLevel | null) => {
    setLevel.mutate(
      { channelId, level },
      { onError: () => toast.error("Couldn't change notifications. Try again.") },
    );
    onClose();
  };

  const toggleMute = () => {
    setMuted.mutate({ channelId, muted: !settings.muted });
    onClose();
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const items = [...(menuRef.current?.querySelectorAll<HTMLElement>('[role^="menuitem"]') ?? [])];
    const at = items.indexOf(document.activeElement as HTMLElement);
    const step = e.key === 'ArrowDown' ? 1 : e.key === 'ArrowUp' ? -1 : 0;
    if (e.key === 'Home') items[0]?.focus();
    else if (e.key === 'End') items[items.length - 1]?.focus();
    else if (step) items[(at + step + items.length) % items.length]?.focus();
    else return;
    e.preventDefault();
  };

  const options: { level: NotifyLevel | null; label: string; qa: string }[] = [
    { level: null, label: `Default (${NOTIFY_LEVEL_LABELS[settings.defaultLevel]})`, qa: 'default' },
    ...LEVELS.map((level) => ({ level, label: NOTIFY_LEVEL_LABELS[level], qa: level })),
  ];

  return (
    <AnchoredPopover
      anchorRef={anchorRef}
      onClose={onClose}
      dataQa="notification-menu"
      className="w-64 bg-surface border border-line rounded-lg shadow-xl py-1"
    >
      <div ref={menuRef} role="menu" aria-label="Notification settings" onKeyDown={onKeyDown}>
        <div className="px-3 py-1 text-xs text-muted" aria-hidden="true">
          Notify me about
        </div>
        {options.map(({ level, label, qa }) => {
          const checked = settings.stored === level;
          return (
            <button
              key={qa}
              type="button"
              role="menuitemradio"
              aria-checked={checked}
              tabIndex={-1}
              data-autofocus={checked || undefined}
              onClick={() => choose(level)}
              data-qa={`notify-level-${qa}`}
              className="w-full px-3 py-1.5 text-left text-sm text-fg-dim hover:bg-raised focus-visible:bg-raised focus-visible:outline-none flex items-center gap-2 cursor-pointer"
            >
              <Check className={`w-4 h-4 shrink-0 ${checked ? 'text-accent' : 'invisible'}`} />
              {label}
            </button>
          );
        })}
        {!direct && (
          <>
            <div role="separator" className="my-1 border-t border-line" />
            <button
              type="button"
              role="menuitemcheckbox"
              aria-checked={settings.muted}
              tabIndex={-1}
              onClick={toggleMute}
              data-qa="notify-mute"
              className="w-full px-3 py-1.5 text-left text-sm text-fg-dim hover:bg-raised focus-visible:bg-raised focus-visible:outline-none flex items-center gap-2 cursor-pointer"
            >
              {settings.muted ? (
                <Bell className="w-4 h-4 shrink-0" />
              ) : (
                <BellOff className="w-4 h-4 shrink-0" />
              )}
              {settings.muted ? 'Unmute channel' : 'Mute channel'}
            </button>
          </>
        )}
      </div>
    </AnchoredPopover>
  );
}

interface ButtonProps extends Target {
  small?: boolean;
  className?: string;
}

export function NotificationLevelButton({ channelId, direct, small = false, className = '' }: ButtonProps) {
  const [open, setOpen] = useState(false);
  const anchorRef = useRef<HTMLButtonElement>(null);
  const { effective, muted } = useNotifySettings({ channelId, direct });
  const Icon = muted || effective === 'nothing' ? BellOff : effective === 'all' ? BellRing : Bell;
  return (
    <>
      <IconButton
        ref={anchorRef}
        label="Notification settings"
        icon={<Icon className={small ? 'w-3 h-3' : 'w-4 h-4'} />}
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen((current) => !current)}
        data-qa="notification-menu-open"
        className={`${small ? 'h-6 w-6' : ''} ${className}`}
      />
      {open && (
        <NotificationLevelMenu
          channelId={channelId}
          direct={direct}
          anchorRef={anchorRef}
          onClose={() => setOpen(false)}
        />
      )}
    </>
  );
}
