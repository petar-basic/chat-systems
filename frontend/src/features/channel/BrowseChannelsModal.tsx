import { useMemo, useState } from 'react';
import { Button } from '@/shared/components/Button/Button';
import { TextInput } from '@/shared/components/Form/TextInput';
import { Select } from '@/shared/components/Form/Select';
import { Spinner } from '@/shared/components/Spinner/Spinner';
import { toUserMessage } from '@/lib/errors';
import { Check, Hash, Lock, Plus, Search } from 'lucide-react';
import { Modal } from '@/shared/components/Modal/Modal';
import {
  useBrowsableChannels,
  useJoinChannel,
  useLeaveChannel,
  type BrowsableChannel,
} from '@/hooks/queries/useChannels';

type SortKey = 'name' | 'members' | 'newest';

const SORTS: Record<SortKey, (a: BrowsableChannel, b: BrowsableChannel) => number> = {
  name: (a, b) => (a.name ?? '').localeCompare(b.name ?? ''),
  members: (a, b) => b.member_count - a.member_count || (a.name ?? '').localeCompare(b.name ?? ''),
  newest: (a, b) => b.created_at.localeCompare(a.created_at),
};

interface Props {
  workspaceId: string;
  instanceUrl?: string;
  currentUserId: string | undefined;
  onClose: () => void;
  onOpenChannel: (channelId: string) => void;
  onCreateChannel?: () => void;
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
  const about = channel.topic || channel.description;
  const Icon = channel.channel_type === 'private' ? Lock : Hash;
  return (
    <div
      data-qa="browse-channel-row"
      data-channel-id={channel.id}
      className="group flex items-center gap-2 rounded-lg px-3 hover:bg-raised/40 focus-within:bg-raised/40"
    >
      <button
        type="button"
        onClick={channel.is_member ? onOpen : onJoin}
        disabled={!channel.is_member && busy}
        data-qa={channel.is_member ? 'browse-channel-open' : undefined}
        className="flex min-w-0 flex-1 items-center gap-3 py-2.5 text-left focus:outline-none"
      >
        <Icon className="h-4 w-4 shrink-0 text-muted" />
        <span className="min-w-0 flex-1">
          <span className="block truncate text-sm font-medium text-fg-soft">{channel.name || 'Channel'}</span>
          <span className="block truncate text-xs text-muted">
            {channel.is_member && (
              <span className="text-success">
                <Check className="mr-0.5 inline h-3 w-3 align-[-2px]" />
                Joined ·{' '}
              </span>
            )}
            {channel.member_count} {channel.member_count === 1 ? 'member' : 'members'}
            {about ? ` · ${about}` : ''}
          </span>
        </span>
      </button>
      <Button
        size="sm"
        onClick={channel.is_member ? onLeave : onJoin}
        disabled={busy}
        data-qa={channel.is_member ? 'browse-channel-leave' : 'browse-channel-join'}
        aria-label={`${channel.is_member ? 'Leave' : 'Join'} ${channel.name || 'channel'}`}
        className="shrink-0 transition-opacity [@media(hover:hover)]:opacity-0 group-hover:opacity-100 group-focus-within:opacity-100"
      >
        {channel.is_member ? 'Leave' : 'Join'}
      </Button>
    </div>
  );
}

export default function BrowseChannelsModal({
  workspaceId,
  instanceUrl,
  currentUserId,
  onClose,
  onOpenChannel,
  onCreateChannel,
}: Props) {
  const [query, setQuery] = useState('');
  const [sort, setSort] = useState<SortKey>('name');
  const [error, setError] = useState<string | null>(null);
  const { data: channels = [], isLoading, isError } = useBrowsableChannels(workspaceId, instanceUrl);
  const join = useJoinChannel(workspaceId, instanceUrl);
  const leave = useLeaveChannel(workspaceId, currentUserId ?? '', instanceUrl);

  const matches = useMemo(() => {
    const q = query.trim().toLowerCase();
    const found = q
      ? channels.filter(
          (c) =>
            (c.name ?? '').toLowerCase().includes(q) ||
            (c.topic ?? '').toLowerCase().includes(q) ||
            (c.description ?? '').toLowerCase().includes(q),
        )
      : channels;
    return [...found].sort(SORTS[sort]);
  }, [channels, query, sort]);

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
      title="Browse channels"
      onClose={onClose}
      dataQa="browse-channels-modal"
      size="lg"
      headerActions={
        onCreateChannel && (
          <Button
            size="sm"
            icon={<Plus className="h-3.5 w-3.5" />}
            onClick={onCreateChannel}
            data-qa="browse-channels-create"
          >
            Create channel
          </Button>
        )
      }
    >
      <div className="sticky top-0 z-10 -mx-6 -mt-1 bg-surface px-6 pt-1 pb-3">
        <div className="flex gap-2">
          <TextInput
            label="Search channels"
            labelHidden
            leading={<Search className="h-4 w-4" />}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search channels…"
            dataQa="browse-channels-search"
            className="flex-1"
          />
          <Select
            label="Sort channels"
            labelHidden
            value={sort}
            onChange={(e) => setSort(e.target.value as SortKey)}
            dataQa="browse-channels-sort"
            className="w-40"
          >
            <option value="name">Name (A–Z)</option>
            <option value="members">Most members</option>
            <option value="newest">Newest</option>
          </Select>
        </div>
        {!isLoading && !isError && (
          <p className="mt-3 text-xs text-muted" data-qa="browse-channels-count" aria-live="polite">
            {matches.length} {matches.length === 1 ? 'channel' : 'channels'}
          </p>
        )}
        {error && (
          <p role="alert" className="mt-2 text-xs text-danger">
            {error}
          </p>
        )}
      </div>

      <div className="-mx-3">
        {isLoading ? (
          <div className="flex justify-center py-8 text-accent">
            <Spinner />
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
    </Modal>
  );
}
