import { useRef, useState, type ReactNode } from 'react';
import { Hash, Lock, Search, Pin, Users, Menu, Info, Plug, MoreHorizontal, Files, Bell } from 'lucide-react';
import { useOnClickOutside } from '@/shared/hooks/useOnClickOutside';
import { IconButton } from '@/shared/components/IconButton/IconButton';
import { useChannelMembers, useChannelPins } from '@/features/channel/hooks/useChannels';
import type { Channel } from '@/stores/workspace';
import { HuddleBar } from '@/features/huddle';
import { useChannelModeration } from './hooks/useChannelModeration';
import { useHookedChannels } from '@/features/workspace';
import { ChannelDetailsModal, type ChannelDetailsTab } from './details/ChannelDetailsModal';
import { NotificationLevelButton, NotificationLevelMenu } from '@/features/notifications';

interface Props {
  channel: Channel | null;
  showSearch: boolean;
  showPins: boolean;
  showFiles: boolean;
  showChannelMembers: boolean;
  onToggleSearch: () => void;
  onTogglePins: () => void;
  onToggleFiles: () => void;
  onToggleChannelMembers: () => void;
  onOpenNav?: () => void;
  onOpenWorkflow?: (workflowId: string) => void;
}

export default function ChannelHeader({
  channel,
  showSearch,
  showPins,
  showFiles,
  showChannelMembers,
  onToggleSearch,
  onTogglePins,
  onToggleFiles,
  onToggleChannelMembers,
  onOpenNav,
  onOpenWorkflow,
}: Props) {
  const { canModerate } = useChannelModeration(channel?.id ?? null);
  const { data: hookedChannels } = useHookedChannels(channel?.workspace_id ?? null);
  const [details, setDetails] = useState<ChannelDetailsTab | null>(null);
  const { data: members } = useChannelMembers(channel?.id ?? null);
  const { data: pins } = useChannelPins(channel?.id ?? null);
  const memberCount = members?.length;
  const pinCount = pins?.length ?? 0;
  const [overflowOpen, setOverflowOpen] = useState(false);
  const [notificationsOpen, setNotificationsOpen] = useState(false);
  const overflowRef = useRef<HTMLDivElement>(null);
  useOnClickOutside(overflowRef, () => setOverflowOpen(false), overflowOpen);
  const isForwarded = !!channel && !!hookedChannels?.has(channel.id);

  return (
    <div
      className="h-14 px-4 flex items-center gap-2 border-b border-line/50 bg-surface/30 shrink-0"
      data-qa="channel-header"
    >
      {onOpenNav && (
        <button
          onClick={onOpenNav}
          aria-label="Open navigation"
          data-qa="mobile-nav-toggle"
          className="lg:hidden p-1.5 -ml-1 rounded-lg text-muted hover:text-fg hover:bg-raised/50"
        >
          <Menu className="w-5 h-5" />
        </button>
      )}

      {channel && (
        <>
          {channel.channel_type === 'private' ? (
            <Lock className="w-4 h-4 text-muted shrink-0" />
          ) : (
            <Hash className="w-4 h-4 text-muted shrink-0" />
          )}
          <span className="font-semibold truncate" data-qa="channel-header-name">
            {channel.name}
          </span>
          {isForwarded && (
            <button
              type="button"
              onClick={() => setDetails('integrations')}
              data-qa="channel-integration-indicator"
              title="An integration forwards messages from this channel to an external URL"
              className="flex items-center gap-1 shrink-0 px-1.5 py-0.5 rounded bg-amber-500/10 text-warning text-[11px] cursor-pointer hover:bg-amber-500/20"
            >
              <Plug className="w-3 h-3" />
              Integration
            </button>
          )}
          {channel.topic && (
            <span className="text-sm text-muted ml-2 truncate hidden sm:inline">— {channel.topic}</span>
          )}

          <div className="ml-auto flex items-center gap-1">
            {!channel.is_archived && <HuddleBar channelId={channel.id} />}
            <NotificationLevelButton channelId={channel.id} direct={false} className="max-sm:hidden" />
            <HeaderToggle
              label="Channel members"
              icon={<Users className="w-4 h-4" />}
              count={memberCount ?? '—'}
              pressed={showChannelMembers}
              onClick={onToggleChannelMembers}
              dataQa="channel-header-members"
            />
            <HeaderToggle
              label="Pinned messages"
              icon={<Pin className="w-4 h-4" />}
              count={pinCount || undefined}
              pressed={showPins}
              onClick={onTogglePins}
              dataQa="channel-header-pins"
            />
            <HeaderToggle
              label="Files"
              icon={<Files className="w-4 h-4" />}
              pressed={showFiles}
              onClick={onToggleFiles}
              dataQa="channel-header-files"
            />
            <IconButton
              label="Search"
              size="md"
              icon={<Search className="w-4 h-4" />}
              pressed={showSearch}
              onClick={onToggleSearch}
              data-qa="channel-header-search"
            />

            <div className="relative" ref={overflowRef}>
              <IconButton
                label="More channel actions"
                size="md"
                icon={<MoreHorizontal className="w-4 h-4" />}
                onClick={() => setOverflowOpen((open) => !open)}
                aria-expanded={overflowOpen}
                data-qa="channel-actions-more"
              />
              {overflowOpen && (
                <div className="absolute right-0 top-full mt-1 w-48 bg-surface border border-line rounded-lg shadow-xl z-20 py-1">
                  <MenuItem
                    icon={<Users className="w-4 h-4" />}
                    label="Members"
                    phoneOnly
                    onClick={() => {
                      onToggleChannelMembers();
                      setOverflowOpen(false);
                    }}
                  />
                  <MenuItem
                    icon={<Pin className="w-4 h-4" />}
                    label="Pinned"
                    phoneOnly
                    onClick={() => {
                      onTogglePins();
                      setOverflowOpen(false);
                    }}
                  />
                  <MenuItem
                    icon={<Files className="w-4 h-4" />}
                    label="Files"
                    phoneOnly
                    onClick={() => {
                      onToggleFiles();
                      setOverflowOpen(false);
                    }}
                  />
                  <MenuItem
                    icon={<Bell className="w-4 h-4" />}
                    label="Notification settings"
                    phoneOnly
                    onClick={() => {
                      setNotificationsOpen(true);
                      setOverflowOpen(false);
                    }}
                  />
                  <MenuItem
                    icon={<Info className="w-4 h-4" />}
                    label="Channel details"
                    dataQa="channel-details-open"
                    onClick={() => {
                      setDetails('about');
                      setOverflowOpen(false);
                    }}
                  />
                </div>
              )}
            </div>
          </div>

          {notificationsOpen && (
            <NotificationLevelMenu
              channelId={channel.id}
              direct={false}
              anchorRef={overflowRef}
              onClose={() => setNotificationsOpen(false)}
            />
          )}

          {details && (
            <ChannelDetailsModal
              channel={channel}
              canModerate={canModerate && !channel.is_archived}
              initialTab={details}
              onOpenWorkflow={onOpenWorkflow}
              onClose={() => setDetails(null)}
            />
          )}
        </>
      )}
    </div>
  );
}

function HeaderToggle({
  label,
  icon,
  count,
  pressed,
  onClick,
  dataQa,
}: {
  label: string;
  icon: ReactNode;
  count?: number | string;
  pressed: boolean;
  onClick: () => void;
  dataQa: string;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-label={label}
      title={label}
      aria-pressed={pressed}
      data-qa={dataQa}
      className="max-sm:hidden inline-flex h-8 min-w-8 shrink-0 items-center justify-center gap-1 rounded-md px-1.5 text-xs text-muted transition cursor-pointer hover:text-fg hover:bg-raised/60 aria-pressed:bg-raised aria-pressed:text-fg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-purple-500"
    >
      {icon}
      {count !== undefined && <span className="tabular-nums">{count}</span>}
    </button>
  );
}

function MenuItem({
  icon,
  label,
  phoneOnly = false,
  dataQa,
  onClick,
}: {
  icon: ReactNode;
  label: string;
  phoneOnly?: boolean;
  dataQa?: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      data-qa={dataQa}
      className={`w-full px-4 py-2 text-left text-sm text-fg-dim hover:bg-raised flex items-center gap-2 cursor-pointer ${phoneOnly ? 'sm:hidden' : ''}`}
      onClick={onClick}
    >
      {icon} {label}
    </button>
  );
}
