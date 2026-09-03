import { useRef, useState } from 'react';
import { Bell, BellRing, Bookmark, Clock, LogOut, User } from 'lucide-react';
import { useOnClickOutside } from '@/shared/hooks/useOnClickOutside';
import { Avatar } from '@/shared/components/Avatar/Avatar';
import { displayNameOf } from '@/lib/userHelpers';

interface Props {
  user: { id: string; display_name: string; email: string; avatar_url: string | null } | null;
  statusEmoji: string | null | undefined;
  statusText: string | null | undefined;
  unreadNotifCount: number;
  onOpenProfile: () => void;
  onOpenScheduled: () => void;
  onOpenSaved: () => void;
  onOpenReminders: () => void;
  onOpenNotifications: () => void;
  onLogout: () => void;
}

const ITEM =
  'w-full px-4 py-2 text-left text-sm text-fg-dim hover:bg-raised flex items-center gap-2 cursor-pointer';

export function YouMenu({
  user,
  statusEmoji,
  statusText,
  unreadNotifCount,
  onOpenProfile,
  onOpenScheduled,
  onOpenSaved,
  onOpenReminders,
  onOpenNotifications,
  onLogout,
}: Props) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useOnClickOutside(ref, () => setOpen(false), open);

  const pick = (action: () => void) => () => {
    action();
    setOpen(false);
  };

  return (
    <div className="px-3 py-3 border-t border-line/50 flex items-center gap-2">
      <button
        onClick={onOpenProfile}
        aria-label="Edit profile"
        data-qa="sidebar-profile-avatar"
        className="rounded-full shrink-0 hover:ring-2 hover:ring-purple-400 transition cursor-pointer"
        title="Edit profile"
      >
        <Avatar
          userId={user?.id ?? ''}
          name={displayNameOf(user?.display_name)}
          avatarUrl={user?.avatar_url}
        />
      </button>
      <div className="relative flex-1 min-w-0" ref={ref}>
        <button
          onClick={() => setOpen((current) => !current)}
          data-qa="open-you-menu"
          className="w-full min-w-0 text-left hover:bg-raised/30 rounded px-1 -mx-1 transition cursor-pointer"
          title="You"
        >
          <div className="text-sm font-medium truncate">
            {user?.display_name}
            {statusEmoji && (
              <span className="ml-1.5" data-qa="own-status-emoji">
                {statusEmoji}
              </span>
            )}
          </div>
          <div className="text-xs text-muted truncate">{statusText || user?.email}</div>
        </button>

        {/* Saved, scheduled and reminders follow the person, not the
            workspace: they are the same list whichever workspace is open,
            so they belong here rather than under a workspace's name. */}
        {open && (
          <div
            className="absolute bottom-full left-0 mb-1 w-52 bg-surface border border-line rounded-lg shadow-xl z-20 py-1"
            data-qa="you-menu"
          >
            <button className={ITEM} data-qa="open-profile" onClick={pick(onOpenProfile)}>
              <User className="w-4 h-4 shrink-0" /> Profile &amp; settings
            </button>
            <div className="my-1 h-px bg-raised" />
            <button className={ITEM} data-qa="open-scheduled" onClick={pick(onOpenScheduled)}>
              <Clock className="w-4 h-4" /> Scheduled
            </button>
            <button className={ITEM} data-qa="open-saved" onClick={pick(onOpenSaved)}>
              <Bookmark className="w-4 h-4" /> Saved
            </button>
            <button className={ITEM} data-qa="open-reminders" onClick={pick(onOpenReminders)}>
              <BellRing className="w-4 h-4" /> Reminders
            </button>
          </div>
        )}
      </div>
      <button
        onClick={onOpenNotifications}
        className="relative text-muted hover:text-fg transition cursor-pointer"
        title="Notifications"
      >
        <Bell className="w-4 h-4" />
        {unreadNotifCount > 0 && (
          <span className="absolute -top-1.5 -right-1.5 min-w-[14px] h-3.5 px-0.5 bg-red-500 text-white text-[9px] font-bold rounded-full flex items-center justify-center leading-none">
            {unreadNotifCount > 99 ? '99+' : unreadNotifCount}
          </span>
        )}
      </button>
      <button
        onClick={onLogout}
        className="text-muted hover:text-danger transition cursor-pointer"
        title="Sign out"
      >
        <LogOut className="w-4 h-4" />
      </button>
    </div>
  );
}
