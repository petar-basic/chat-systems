import { useState, type FormEvent } from 'react';
import { Button } from '@/shared/components/Button/Button';
import { DialogHeader } from '@/shared/components/Dialog/Dialog';
import { Archive } from 'lucide-react';
import { Modal } from '@/shared/components/Modal/Modal';
import type { Channel } from '@/stores/workspace';
import { useUpdateChannel, useArchiveChannel } from '@/hooks/queries/useChannels';

interface Props {
  channel: Channel;
  workspaceId: string;
  onClose: () => void;
  onArchived: () => void;
}

export default function ChannelSettingsModal({ channel, workspaceId, onClose, onArchived }: Props) {
  const [name, setName] = useState(channel.name ?? '');
  const [topic, setTopic] = useState(channel.topic ?? '');
  const [description, setDescription] = useState(channel.description ?? '');
  const [announcementOnly, setAnnouncementOnly] = useState(channel.settings?.post_policy === 'moderators');
  const [confirmArchive, setConfirmArchive] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const updateChannel = useUpdateChannel(workspaceId, channel.id);
  const archiveChannel = useArchiveChannel(workspaceId);

  const failWith = (fallback: string) => (err: unknown) =>
    setError((err as { message?: string })?.message || fallback);

  const handleSave = async (e: FormEvent) => {
    e.preventDefault();
    if (!name.trim()) return;
    setError(null);
    try {
      await updateChannel.mutateAsync({
        name: name.trim(),
        topic: topic.trim(),
        description: description.trim(),
        post_policy: announcementOnly ? 'moderators' : 'everyone',
      });
      onClose();
    } catch (err: unknown) {
      failWith('Failed to save the channel')(err);
    }
  };

  const handleArchive = async () => {
    setError(null);
    try {
      await archiveChannel.mutateAsync(channel.id);
      onArchived();
      onClose();
    } catch (err: unknown) {
      failWith('Failed to archive the channel')(err);
    }
  };

  return (
    <Modal
      title="Channel settings"
      onClose={onClose}
      dataQa="channel-settings-modal"
      className="bg-surface border border-line rounded-2xl p-6 w-full max-w-md shadow-2xl"
    >
      <DialogHeader title="Channel settings" onClose={onClose} />

      <form onSubmit={handleSave} className="space-y-4">
        <div>
          <label htmlFor="channel-name" className="block text-sm font-medium text-fg-dim mb-1.5">
            Name
          </label>
          <input
            id="channel-name"
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            data-qa="channel-settings-name"
            className="w-full px-3 py-2.5 bg-raised/50 border border-line-strong rounded-lg text-fg text-sm placeholder-muted focus:outline-none focus:ring-2 focus:ring-purple-500"
            required
          />
        </div>

        <div>
          <label htmlFor="channel-topic" className="block text-sm font-medium text-fg-dim mb-1.5">
            Topic
          </label>
          <input
            id="channel-topic"
            type="text"
            value={topic}
            onChange={(e) => setTopic(e.target.value)}
            placeholder="What is this channel about?"
            data-qa="channel-settings-topic"
            className="w-full px-3 py-2.5 bg-raised/50 border border-line-strong rounded-lg text-fg text-sm placeholder-muted focus:outline-none focus:ring-2 focus:ring-purple-500"
          />
        </div>

        <div>
          <label htmlFor="channel-description" className="block text-sm font-medium text-fg-dim mb-1.5">
            Description
          </label>
          <textarea
            id="channel-description"
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            rows={3}
            data-qa="channel-settings-description"
            className="w-full px-3 py-2.5 bg-raised/50 border border-line-strong rounded-lg text-fg text-sm placeholder-muted focus:outline-none focus:ring-2 focus:ring-purple-500 resize-none"
          />
        </div>

        <label className="flex items-start gap-3 cursor-pointer" data-qa="channel-settings-announcement">
          <input
            type="checkbox"
            checked={announcementOnly}
            onChange={(e) => setAnnouncementOnly(e.target.checked)}
            className="mt-0.5 w-4 h-4 accent-purple-500"
          />
          <span>
            <span className="block text-sm font-medium text-fg-dim">Announcement channel</span>
            <span className="block text-xs text-muted">
              Only workspace admins and this channel's admins can post. Everyone else can still read and
              react.
            </span>
          </span>
        </label>

        {error && (
          <div className="text-sm text-danger bg-red-500/10 border border-red-500/30 rounded-lg px-3 py-2">
            {error}
          </div>
        )}

        <div className="flex items-center justify-between gap-2 pt-2">
          {confirmArchive ? (
            <div className="flex items-center gap-2 text-xs">
              <span className="text-danger">Archive this channel?</span>
              <Button
                variant="danger"
                size="sm"
                onClick={handleArchive}
                loading={archiveChannel.isPending}
                data-qa="channel-settings-archive-confirm"
              >
                Archive
              </Button>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => setConfirmArchive(false)}
                data-qa="channel-settings-archive-cancel"
              >
                Keep channel
              </Button>
            </div>
          ) : (
            <Button
              variant="ghost"
              size="sm"
              icon={<Archive className="h-3.5 w-3.5" />}
              onClick={() => setConfirmArchive(true)}
              data-qa="channel-settings-archive"
              className="-ml-2.5 hover:text-danger"
            >
              Archive channel
            </Button>
          )}

          <div className="flex gap-2">
            <Button variant="ghost" onClick={onClose}>
              Cancel
            </Button>
            <Button
              type="submit"
              variant="primary"
              loading={updateChannel.isPending}
              disabled={!name.trim()}
              data-qa="channel-settings-save"
            >
              Save
            </Button>
          </div>
        </div>
      </form>
    </Modal>
  );
}
