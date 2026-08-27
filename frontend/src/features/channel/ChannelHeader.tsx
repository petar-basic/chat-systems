import { useRef, useState } from 'react';
import { Hash, Lock, Search, Pin, Users, Menu, Settings, Plug, MoreHorizontal } from 'lucide-react';
import { useOnClickOutside } from '@/shared/hooks/useOnClickOutside';
import { useNavigate } from 'react-router';
import type { Channel } from '@/stores/workspace';
import { HuddleBar } from '@/features/huddle';
import { useChannelModeration } from './hooks/useChannelModeration';
import { useHookedChannels } from '@/hooks/queries/useHooks';
import ChannelSettingsModal from './ChannelSettingsModal';

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
  const navigate = useNavigate();
  const { canModerate } = useChannelModeration(channel?.id ?? null);
  const { data: hookedChannels } = useHookedChannels(channel?.workspace_id ?? null);
  const [showSettings, setShowSettings] = useState(false);
  const [overflowOpen, setOverflowOpen] = useState(false);
  const overflowRef = useRef<HTMLDivElement>(null);
  useOnClickOutside(overflowRef, () => setOverflowOpen(false), overflowOpen);
  const isForwarded = !!channel && !!hookedChannels?.has(channel.id);

  return (
    <div className="h-14 px-4 flex items-center gap-2 border-b border-slate-700/50 bg-slate-800/30 shrink-0">
      {onOpenNav && (
        <button
          onClick={onOpenNav}
          aria-label="Open navigation"
          data-qa="mobile-nav-toggle"
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
          <span className="font-semibold truncate" data-qa="channel-header-name">
            {channel.name}
          </span>
          {isForwarded && (
            <span
              data-qa="channel-integration-indicator"
              title="An integration forwards messages from this channel to an external URL"
              className="flex items-center gap-1 shrink-0 px-1.5 py-0.5 rounded bg-amber-500/10 text-amber-400 text-[11px]"
            >
              <Plug className="w-3 h-3" />
              Integration
            </span>
          )}
          {channel.topic && (
            <span className="text-sm text-slate-400 ml-2 truncate hidden sm:inline">— {channel.topic}</span>
          )}

          <div className="ml-auto flex items-center gap-1">
            <HuddleBar channelId={channel.id} />
            {canModerate && (
              <button
                onClick={() => setShowSettings(true)}
                aria-label="Channel settings"
                data-qa="channel-settings-open"
                className="max-sm:hidden p-1.5 rounded-lg transition text-slate-400 hover:text-white hover:bg-slate-700/50"
              >
                <Settings className="w-4 h-4" />
              </button>
            )}
            <button
              onClick={onToggleChannelMembers}
              aria-label="Channel members"
              aria-pressed={showChannelMembers}
              className={`max-sm:hidden p-1.5 rounded-lg transition ${showChannelMembers ? 'bg-slate-700 text-white' : 'text-slate-400 hover:text-white hover:bg-slate-700/50'}`}
            >
              <Users className="w-4 h-4" />
            </button>
            <button
              onClick={onTogglePins}
              aria-label="Pinned messages"
              aria-pressed={showPins}
              className={`max-sm:hidden p-1.5 rounded-lg transition ${showPins ? 'bg-slate-700 text-amber-400' : 'text-slate-400 hover:text-white hover:bg-slate-700/50'}`}
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

            {/* Five unlabelled icons and a channel name do not fit a phone, so
                below `sm` the less-used three fold into one menu. */}
            <div className="relative sm:hidden" ref={overflowRef}>
              <button
                onClick={() => setOverflowOpen((open) => !open)}
                aria-label="More channel actions"
                data-qa="channel-actions-more"
                className="p-1.5 rounded-lg transition text-slate-400 hover:text-white hover:bg-slate-700/50"
              >
                <MoreHorizontal className="w-4 h-4" />
              </button>
              {overflowOpen && (
                <div className="absolute right-0 top-full mt-1 w-48 bg-slate-800 border border-slate-700 rounded-lg shadow-xl z-20 py-1">
                  <button
                    className="w-full px-4 py-2 text-left text-sm text-slate-300 hover:bg-slate-700 flex items-center gap-2 cursor-pointer"
                    onClick={() => {
                      onToggleChannelMembers();
                      setOverflowOpen(false);
                    }}
                  >
                    <Users className="w-4 h-4" /> Members
                  </button>
                  <button
                    className="w-full px-4 py-2 text-left text-sm text-slate-300 hover:bg-slate-700 flex items-center gap-2 cursor-pointer"
                    onClick={() => {
                      onTogglePins();
                      setOverflowOpen(false);
                    }}
                  >
                    <Pin className="w-4 h-4" /> Pinned
                  </button>
                  {canModerate && (
                    <button
                      className="w-full px-4 py-2 text-left text-sm text-slate-300 hover:bg-slate-700 flex items-center gap-2 cursor-pointer"
                      onClick={() => {
                        setShowSettings(true);
                        setOverflowOpen(false);
                      }}
                    >
                      <Settings className="w-4 h-4" /> Channel settings
                    </button>
                  )}
                </div>
              )}
            </div>
          </div>

          {showSettings && (
            <ChannelSettingsModal
              channel={channel}
              workspaceId={channel.workspace_id}
              onClose={() => setShowSettings(false)}
              onArchived={() => navigate(`/app/${channel.workspace_id}`)}
            />
          )}
        </>
      )}
    </div>
  );
}
