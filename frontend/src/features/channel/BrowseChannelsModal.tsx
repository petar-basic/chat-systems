import { useMemo, useState } from 'react';
import { toUserMessage } from '@/lib/errors';
import { Hash, Search } from 'lucide-react';
import { Modal } from '@/shared/components/Modal/Modal';
import {
  useBrowsableChannels,
  useJoinChannel,
  useLeaveChannel,
  type BrowsableChannel,
} from '@/hooks/queries/useChannels';

interface Props {
  workspaceId: string;
  instanceUrl?: string;
  currentUserId: string | undefined;
  onClose: () => void;
  onOpenChannel: (channelId: string) => void;
}

function ChannelRow({
  channel,
  busy,
  onJoin,
  onLeave,
  onOpen,
}: {
  channel: BrowsableChannel;
  busy: boolean;
  onJoin: () => void;
  onLeave: () => void;
  onOpen: () => void;
}) {
  return (
    <div
      data-qa="browse-channel-row"
      data-channel-id={channel.id}
      className="flex items-center gap-3 px-3 py-2.5 rounded-lg hover:bg-raised/30"
    >
      <Hash className="w-4 h-4 text-muted shrink-0" />
      <div className="flex-1 min-w-0">
        <div className="text-sm font-medium text-fg-soft truncate">{channel.name || 'Channel'}</div>
        <div className="text-xs text-muted truncate">
          {channel.member_count} {channel.member_count === 1 ? 'member' : 'members'}
          {channel.topic || channel.description ? ` · ${channel.topic || channel.description}` : ''}
        </div>
      </div>
      {channel.is_member ? (
        <div className="flex items-center gap-1.5 shrink-0">
          <button
            onClick={onOpen}
            data-qa="browse-channel-open"
            className="px-2.5 py-1 text-xs font-medium text-fg-soft bg-raised hover:bg-elevated rounded-lg transition cursor-pointer"
          >
            Open
          </button>
          <button
            onClick={onLeave}
            disabled={busy}
            data-qa="browse-channel-leave"
            className="px-2.5 py-1 text-xs text-muted hover:text-danger transition cursor-pointer disabled:opacity-50"
          >
            Leave
          </button>
        </div>
      ) : (
        <button
          onClick={onJoin}
          disabled={busy}
          data-qa="browse-channel-join"
          className="px-2.5 py-1 text-xs font-medium text-white bg-purple-600 hover:bg-purple-500 rounded-lg transition cursor-pointer disabled:opacity-50 shrink-0"
        >
          Join
        </button>
      )}
    </div>
  );
}

export default function BrowseChannelsModal({
  workspaceId,
  instanceUrl,
  currentUserId,
  onClose,
  onOpenChannel,
}: Props) {
  const [query, setQuery] = useState('');
  const [error, setError] = useState<string | null>(null);
  const { data: channels = [], isLoading, isError } = useBrowsableChannels(workspaceId, instanceUrl);
  const join = useJoinChannel(workspaceId, instanceUrl);
  const leave = useLeaveChannel(workspaceId, currentUserId ?? '', instanceUrl);

  const matches = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return channels;
    return channels.filter(
      (c) =>
        (c.name ?? '').toLowerCase().includes(q) ||
        (c.topic ?? '').toLowerCase().includes(q) ||
        (c.description ?? '').toLowerCase().includes(q),
    );
  }, [channels, query]);

  const handleJoin = async (channelId: string) => {
    setError(null);
    try {
      await join.mutateAsync(channelId);
      onOpenChannel(channelId);
      onClose();
    } catch (err: unknown) {
      setError(toUserMessage(err, 'Failed to join the channel'));
    }
  };

  const handleLeave = async (channelId: string) => {
    if (!currentUserId) return;
    setError(null);
    try {
      await leave.mutateAsync(channelId);
    } catch (err: unknown) {
      setError(toUserMessage(err, 'Failed to leave the channel'));
    }
  };

  const busy = join.isPending || leave.isPending;

  return (
    <Modal
      title="Browse Channels"
      onClose={onClose}
      dataQa="browse-channels-modal"
      className="bg-surface border border-line rounded-2xl p-6 w-full max-w-lg shadow-2xl"
    >
      <h2 className="text-lg font-bold mb-4">Browse Channels</h2>

      <div className="flex items-center gap-2 bg-raised/50 border border-line-strong rounded-lg px-3 py-2 mb-3">
        <Search className="w-4 h-4 text-muted shrink-0" />
        <input
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search channels…"
          aria-label="Search channels"
          data-qa="browse-channels-search"
          className="flex-1 bg-transparent text-fg text-sm placeholder-muted focus:outline-none"
        />
      </div>

      {error && <div className="mb-2 text-xs text-danger">{error}</div>}

      <div className="max-h-80 overflow-y-auto -mx-1 px-1">
        {isLoading ? (
          <div className="flex justify-center py-8">
            <div className="w-5 h-5 border-2 border-purple-500/30 border-t-purple-500 rounded-full animate-spin" />
          </div>
        ) : isError ? (
          <div className="text-center py-8 text-sm text-danger">Failed to load channels</div>
        ) : matches.length === 0 ? (
          <div className="text-center py-8 text-sm text-muted" data-qa="browse-channels-empty">
            No channels found
          </div>
        ) : (
          matches.map((channel) => (
            <ChannelRow
              key={channel.id}
              channel={channel}
              busy={busy}
              onJoin={() => void handleJoin(channel.id)}
              onLeave={() => void handleLeave(channel.id)}
              onOpen={() => {
                onOpenChannel(channel.id);
                onClose();
              }}
            />
          ))
        )}
      </div>

      <div className="mt-4 flex justify-end">
        <button onClick={onClose} className="px-4 py-2 text-muted hover:text-fg transition">
          Close
        </button>
      </div>
    </Modal>
  );
}
