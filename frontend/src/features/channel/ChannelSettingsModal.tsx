import { useState, type FormEvent } from 'react';
import { Button } from '@/shared/components/Button/Button';
import { Archive } from 'lucide-react';
import { Modal } from '@/shared/components/Modal/Modal';
import { ConfirmDialog } from '@/shared/components/ConfirmDialog/ConfirmDialog';
import { Checkbox } from '@/shared/components/Form/Checkbox';
import { TextArea } from '@/shared/components/Form/TextArea';
import { TextInput } from '@/shared/components/Form/TextInput';
import type { Channel } from '@/stores/workspace';
import { useUpdateChannel, useArchiveChannel } from '@/features/channel/hooks/useChannels';
import { useChannelRetention, useSetChannelRetention } from '@/features/workspace/hooks/useRetention';
import { ChannelRetentionField } from './ChannelRetentionField';
import { choiceOf, periodOf, type PeriodChoice } from './channelPeriod';

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
  const [retentionChoice, setRetentionChoice] = useState<PeriodChoice | null>(null);

  const updateChannel = useUpdateChannel(workspaceId, channel.id);
  const archiveChannel = useArchiveChannel(workspaceId);
  const retention = useChannelRetention(channel.id);
  const setRetention = useSetChannelRetention(channel.id);

  const savedChoice = retention.data ? choiceOf(retention.data) : null;
  const choice = retentionChoice ?? savedChoice;
  const period = choice ? periodOf(choice) : undefined;
  const retentionChanged =
    !!retention.data?.overrides_allowed && savedChoice !== null && period !== periodOf(savedChoice);

  const failWith = (fallback: string) => (err: unknown) =>
    setError((err as { message?: string })?.message || fallback);

  const handleSave = async (e: FormEvent) => {
    e.preventDefault();
    if (!name.trim() || period === 'invalid') return;
    setError(null);
    try {
      await updateChannel.mutateAsync({
        name: name.trim(),
        topic: topic.trim(),
        description: description.trim(),
        post_policy: announcementOnly ? 'moderators' : 'everyone',
      });
      if (retentionChanged && period !== undefined) await setRetention.mutateAsync(period);
      onClose();
    } catch (err: unknown) {
      failWith('Failed to save the channel')(err);
    }
  };

  const handleArchive = async () => {
    await archiveChannel.mutateAsync(channel.id);
    onArchived();
    onClose();
  };

  if (confirmArchive) {
    return (
      <ConfirmDialog
        title={`Archive #${channel.name ?? 'channel'}?`}
        description="It leaves the channel list and Browse channels, and nobody can join it any more."
        confirmLabel="Archive channel"
        onConfirm={handleArchive}
        onClose={() => setConfirmArchive(false)}
        dataQa="channel-settings-archive-dialog"
      />
    );
  }

  return (
    <Modal
      title="Channel settings"
      onClose={onClose}
      dataQa="channel-settings-modal"
      onSubmit={handleSave}
      footerStart={
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
      }
      dismissLabel="Cancel"
      actions={
        <Button
          type="submit"
          variant="primary"
          loading={updateChannel.isPending || setRetention.isPending}
          disabled={!name.trim() || period === 'invalid'}
          data-qa="channel-settings-save"
        >
          Save
        </Button>
      }
    >
      <div className="space-y-4">
        <TextInput
          label="Name"
          value={name}
          onChange={(e) => setName(e.target.value)}
          maxLength={80}
          dataQa="channel-settings-name"
          required
        />
        <TextInput
          label="Topic"
          optional
          value={topic}
          onChange={(e) => setTopic(e.target.value)}
          placeholder="What is this channel about?"
          dataQa="channel-settings-topic"
        />
        <TextArea
          label="Description"
          optional
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          dataQa="channel-settings-description"
        />
        <Checkbox
          label="Announcement channel"
          description="Only workspace admins and this channel's admins can post. Everyone else can still read and react."
          checked={announcementOnly}
          onChange={setAnnouncementOnly}
          dataQa="channel-settings-announcement"
        />
        {retention.data && choice && (
          <ChannelRetentionField retention={retention.data} choice={choice} onChange={setRetentionChoice} />
        )}
        {error && (
          <p role="alert" className="text-sm text-danger">
            {error}
          </p>
        )}
      </div>
    </Modal>
  );
}
