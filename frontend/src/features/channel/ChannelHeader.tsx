import { Hash, Lock, Search, Pin, Users, Menu } from 'lucide-react';
import type { Channel } from '@/stores/workspace';
import { HuddleBar } from '@/features/huddle';

interface Props {
  channel: Channel | null;
  showSearch: boolean;
  showPins: boolean;
  showChannelMembers: boolean;
  onToggleSearch: () => void;
  onTogglePins: () => void;
  onToggleChannelMembers: () => void;
  onOpenNav?: () => void;
}

export default function ChannelHeader({
  channel,
  showSearch,
  showPins,
  showChannelMembers,
  onToggleSearch,
  onTogglePins,
  onToggleChannelMembers,
  onOpenNav,
}: Props) {
  return (
    <div className="h-14 px-4 flex items-center gap-2 border-b border-slate-700/50 bg-slate-800/30 shrink-0">
      {onOpenNav && (
        <button
          onClick={onOpenNav}
          aria-label="Open navigation"
          className="lg:hidden p-1.5 -ml-1 rounded-lg text-slate-400 hover:text-white hover:bg-slate-700/50"
        >
          <Menu className="w-5 h-5" />
        </button>
      )}

      {channel && (
        <>
          {channel.channel_type === 'private' ? (
            <Lock className="w-4 h-4 text-slate-400 shrink-0" />
          ) : (
            <Hash className="w-4 h-4 text-slate-400 shrink-0" />
          )}
          <span className="font-semibold truncate">{channel.name}</span>
          {channel.topic && (
            <span className="text-sm text-slate-400 ml-2 truncate hidden sm:inline">— {channel.topic}</span>
          )}

          <div className="ml-auto flex items-center gap-1">
            <HuddleBar channelId={channel.id} />
            <button
              onClick={onToggleChannelMembers}
              aria-label="Channel members"
              aria-pressed={showChannelMembers}
              className={`p-1.5 rounded-lg transition ${showChannelMembers ? 'bg-slate-700 text-white' : 'text-slate-400 hover:text-white hover:bg-slate-700/50'}`}
            >
              <Users className="w-4 h-4" />
            </button>
            <button
              onClick={onTogglePins}
              aria-label="Pinned messages"
              aria-pressed={showPins}
              className={`p-1.5 rounded-lg transition ${showPins ? 'bg-slate-700 text-amber-400' : 'text-slate-400 hover:text-white hover:bg-slate-700/50'}`}
            >
              <Pin className="w-4 h-4" />
            </button>
            <button
              onClick={onToggleSearch}
              aria-label="Search messages"
              aria-pressed={showSearch}
              className={`p-1.5 rounded-lg transition ${showSearch ? 'bg-slate-700 text-white' : 'text-slate-400 hover:text-white hover:bg-slate-700/50'}`}
            >
              <Search className="w-4 h-4" />
            </button>
          </div>
        </>
      )}
    </div>
  );
}
