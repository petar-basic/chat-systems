import { useEffect, useRef, useState } from 'react';
import {
  X,
  Bell,
  MessageSquare,
  AtSign,
  Reply,
  Smile,
  Clock,
  Info,
  Volume2,
  VolumeX,
  Moon,
} from 'lucide-react';
import {
  useNotifications,
  useUnreadNotificationCount,
  useMarkAllNotificationsRead,
  useMarkNotificationsRead,
  useDndStatus,
  useSetDnd,
  type AppNotification,
  type NotificationKind,
} from '../hooks/queries/useNotifications';
import { flattenMentions } from '@/lib/mentions';
import { useNotificationPrefs } from '@/stores/notificationPrefs';
import { useOnClickOutside } from '@/shared/hooks/useOnClickOutside';
import { QueryState } from '@/shared/components/QueryState/QueryState';
import { ActionLabels, EmptyLabels, RELATIVE_TIME_TICK_MS } from '@/shared/constants';

const DND_PRESETS: { label: string; minutes: number }[] = [
  { label: 'For 30 minutes', minutes: 30 },
  { label: 'For 1 hour', minutes: 60 },
  { label: 'For 2 hours', minutes: 120 },
];

interface Props {
  workspaceId: string;
  onClose: () => void;
  onNavigate?: (channelId: string, messageId: string, openThread?: boolean) => void;
}

type Filter = 'all' | 'unread' | 'mention';

const FILTERS: { key: Filter; label: string }[] = [
  { key: 'all', label: 'All' },
  { key: 'unread', label: 'Unread' },
  { key: 'mention', label: 'Mentions' },
];

function notificationIcon(type: NotificationKind) {
  switch (type) {
    case 'mention':
      return <AtSign className="w-3.5 h-3.5 text-danger" />;
    case 'dm':
      return <MessageSquare className="w-3.5 h-3.5 text-accent" />;
    case 'reply':
      return <Reply className="w-3.5 h-3.5 text-info" />;
    case 'reaction':
      return <Smile className="w-3.5 h-3.5 text-warning" />;
    case 'reminder':
      return <Clock className="w-3.5 h-3.5 text-success" />;
    default:
      return <Info className="w-3.5 h-3.5 text-muted" />;
  }
}

function isDndActive(dndUntil: string | null | undefined): boolean {
  return !!dndUntil && new Date(dndUntil).getTime() > Date.now();
}

function snoozeUntilIso(minutes: number): string {
  return new Date(Date.now() + minutes * 60000).toISOString();
}

function tomorrowMorningIso(): string {
  const d = new Date();
  d.setDate(d.getDate() + 1);
  d.setHours(8, 0, 0, 0);
  return d.toISOString();
}

function formatRelativeTime(iso: string) {
  const diff = Date.now() - new Date(iso).getTime();
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return 'just now';
  if (mins < 60) return `${mins}m ago`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 24) return `${hrs}h ago`;
  return `${Math.floor(hrs / 24)}d ago`;
}

function NotificationRow({
  notification,
  onMarkRead,
  onNavigate,
}: {
  notification: AppNotification;
  onMarkRead: (id: string) => void;
  onNavigate?: (channelId: string, messageId: string, openThread?: boolean) => void;
}) {
  const isNavigable = !!(notification.channel_id && notification.message_id);

  const handleClick = () => {
    if (!notification.is_read) onMarkRead(notification.id);
    if (notification.channel_id && notification.message_id && onNavigate) {
      onNavigate(
        notification.channel_id,
        notification.message_id,
        notification.notification_type === 'reply',
      );
    }
  };

  return (
    <button
      onClick={handleClick}
      data-qa="notification-row"
      className={`w-full text-left px-4 py-3 flex items-start gap-3 hover:bg-raised/40 transition border-b border-line/30 last:border-0 ${
        notification.is_read ? 'opacity-50' : ''
      } ${isNavigable ? '' : 'cursor-default'}`}
    >
      <div className="mt-0.5 shrink-0 w-6 h-6 rounded-full bg-raised/60 flex items-center justify-center">
        {notificationIcon(notification.notification_type)}
      </div>
      <div className="flex-1 min-w-0">
        <div className="flex items-center justify-between gap-2 mb-0.5">
          <span className="text-xs font-semibold text-fg-soft truncate">{notification.title}</span>
          <span className="text-xs text-muted shrink-0">{formatRelativeTime(notification.created_at)}</span>
        </div>
        <p className="text-xs text-muted line-clamp-2">{flattenMentions(notification.body)}</p>
      </div>
      {!notification.is_read && <div className="mt-1.5 w-2 h-2 rounded-full bg-purple-500 shrink-0" />}
    </button>
  );
}

export default function NotificationsPanel({ workspaceId, onClose, onNavigate }: Props) {
  const { data: notifications = [], isLoading, isError, refetch } = useNotifications(workspaceId);
  const { data: unreadCount = 0 } = useUnreadNotificationCount(workspaceId);
  const markRead = useMarkNotificationsRead(workspaceId);
  const markAllRead = useMarkAllNotificationsRead(workspaceId);
  const { soundEnabled, toggleSound } = useNotificationPrefs();

  const { data: dndUntil } = useDndStatus();
  const setDnd = useSetDnd();
  const dndActive = isDndActive(dndUntil);
  const [dndMenuOpen, setDndMenuOpen] = useState(false);
  const dndRef = useRef<HTMLDivElement>(null);
  useOnClickOutside(dndRef, () => setDndMenuOpen(false), dndMenuOpen);

  const snooze = (minutes: number) => {
    setDnd.mutate(snoozeUntilIso(minutes));
    setDndMenuOpen(false);
  };
  const snoozeUntilTomorrow = () => {
    setDnd.mutate(tomorrowMorningIso());
    setDndMenuOpen(false);
  };
  const clearDnd = () => {
    setDnd.mutate(null);
    setDndMenuOpen(false);
  };

  const [filter, setFilter] = useState<Filter>('all');
  const [, setTick] = useState(0);
  useEffect(() => {
    const id = setInterval(() => setTick((t) => t + 1), RELATIVE_TIME_TICK_MS);
    return () => clearInterval(id);
  }, []);

  const filtered = notifications.filter((n) => {
    if (filter === 'unread') return !n.is_read;
    if (filter === 'mention') return n.notification_type === 'mention';
    return true;
  });

  return (
    <div
      className="w-full lg:w-80 max-lg:fixed max-lg:inset-0 max-lg:z-40 bg-surface/95 border-l border-line/50 flex flex-col h-full"
      data-qa="notifications-panel"
    >
      <div className="h-14 px-4 flex items-center justify-between border-b border-line/50 shrink-0">
        <div className="flex items-center gap-2">
          <Bell className="w-4 h-4 text-fg-dim" />
          <h2 className="font-semibold text-fg">Notifications</h2>
          {unreadCount > 0 && (
            <span className="px-1.5 py-0.5 bg-red-500 text-white text-[10px] font-bold rounded-full">
              {unreadCount > 99 ? '99+' : unreadCount}
            </span>
          )}
        </div>
        <div className="flex items-center gap-2">
          <div className="relative" ref={dndRef}>
            <button
              onClick={() => setDndMenuOpen((v) => !v)}
              aria-label="Do not disturb"
              title={dndActive ? 'Do Not Disturb is on' : 'Pause notifications'}
              className={`transition ${dndActive ? 'text-accent' : 'text-muted hover:text-fg'}`}
            >
              <Moon className="w-4 h-4" />
            </button>
            {dndMenuOpen && (
              <div className="absolute right-0 top-full mt-1 w-44 bg-surface border border-line rounded-lg shadow-xl z-10 py-1">
                {dndActive ? (
                  <button
                    onClick={clearDnd}
                    className="w-full px-3 py-2 text-left text-sm text-accent-soft hover:bg-raised"
                  >
                    Turn off Do Not Disturb
                  </button>
                ) : (
                  <>
                    <div className="px-3 py-1 text-[10px] uppercase tracking-wide text-subtle">
                      Pause notifications
                    </div>
                    {DND_PRESETS.map((p) => (
                      <button
                        key={p.minutes}
                        onClick={() => snooze(p.minutes)}
                        className="w-full px-3 py-1.5 text-left text-sm text-fg-dim hover:bg-raised"
                      >
                        {p.label}
                      </button>
                    ))}
                    <button
                      onClick={snoozeUntilTomorrow}
                      className="w-full px-3 py-1.5 text-left text-sm text-fg-dim hover:bg-raised"
                    >
                      Until tomorrow
                    </button>
                  </>
                )}
              </div>
            )}
          </div>
          <button
            onClick={toggleSound}
            aria-label={soundEnabled ? 'Mute notification sound' : 'Unmute notification sound'}
            title={soundEnabled ? 'Mute sound' : 'Unmute sound'}
            className="text-muted hover:text-fg transition"
          >
            {soundEnabled ? <Volume2 className="w-4 h-4" /> : <VolumeX className="w-4 h-4 text-danger" />}
          </button>
          {unreadCount > 0 && (
            <button
              onClick={() => markAllRead.mutate()}
              disabled={markAllRead.isPending}
              className="text-xs text-accent hover:text-accent-soft transition disabled:opacity-50"
            >
              {ActionLabels.MarkAllRead}
            </button>
          )}
          <button
            onClick={onClose}
            aria-label="Close notifications"
            className="text-muted hover:text-fg transition"
          >
            <X className="w-5 h-5" />
          </button>
        </div>
      </div>
      <div className="flex items-center gap-1 px-3 py-2 border-b border-line/40 shrink-0">
        {FILTERS.map((f) => (
          <button
            key={f.key}
            onClick={() => setFilter(f.key)}
            className={`px-2.5 py-1 text-xs rounded-md transition ${
              filter === f.key ? 'bg-raised text-fg' : 'text-muted hover:text-fg-soft'
            }`}
          >
            {f.label}
          </button>
        ))}
      </div>
      <div className="flex-1 overflow-y-auto">
        <QueryState
          isLoading={isLoading}
          isError={isError}
          isEmpty={filtered.length === 0}
          onRetry={() => void refetch()}
          empty={
            <>
              <Bell className="w-10 h-10 mb-3 text-faint" />
              <p className="text-sm font-medium">{EmptyLabels.NoNotifications}</p>
              <p className="text-xs mt-1">{EmptyLabels.NoNotificationsHint}</p>
            </>
          }
        >
          {filtered.map((n) => (
            <NotificationRow
              key={n.id}
              notification={n}
              onMarkRead={(id) => markRead.mutate([id])}
              onNavigate={onNavigate}
            />
          ))}
        </QueryState>
      </div>
    </div>
  );
}
