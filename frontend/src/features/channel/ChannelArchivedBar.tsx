import { Archive } from 'lucide-react';
import { Button } from '@/shared/components/Button/Button';
import { toast } from '@/shared/components/Toast';
import { toUserMessage } from '@/lib/errors';
import type { Channel } from '@/stores/workspace';
import { useUnarchiveChannel } from './hooks/useChannels';

interface Props {
  channel: Channel;
  canUnarchive: boolean;
}

export function ChannelArchivedBar({ channel, canUnarchive }: Props) {
  const unarchive = useUnarchiveChannel(channel.workspace_id);
  const onUnarchive = () =>
    unarchive.mutate(channel.id, {
      onError: (err) => toast.error(toUserMessage(err, 'Failed to unarchive the channel')),
    });

  return (
    <div
      data-qa="channel-archived"
      className="mx-4 mb-4 px-4 py-3 rounded-lg bg-surface/60 border border-line text-sm text-muted flex items-center gap-3"
    >
      <Archive className="w-4 h-4 shrink-0" />
      <span className="flex-1 min-w-0">
        <span className="font-semibold text-fg-soft">#{channel.name}</span> is archived. Its history stays
        readable, and nothing new can be posted.
      </span>
      {canUnarchive && (
        <Button
          variant="primary"
          size="sm"
          loading={unarchive.isPending}
          onClick={onUnarchive}
          data-qa="channel-unarchive"
        >
          Unarchive
        </Button>
      )}
    </div>
  );
}
