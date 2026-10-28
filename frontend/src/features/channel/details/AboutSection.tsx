import { useState, type FormEvent } from 'react';
import { Archive } from 'lucide-react';
import { Button } from '@/shared/components/Button/Button';
import { ConfirmDialog } from '@/shared/components/ConfirmDialog/ConfirmDialog';
import { TextArea } from '@/shared/components/Form/TextArea';
import { TextInput } from '@/shared/components/Form/TextInput';
import type { Channel } from '@/stores/workspace';
import { useArchiveChannel, useUpdateChannel } from '../hooks/useChannels';

interface Props {
  channel: Channel;
  canModerate: boolean;
  onClose: () => void;
}

export function AboutSection({ channel, canModerate, onClose }: Props) {
  if (!canModerate) return <AboutText channel={channel} />;
  return <AboutForm channel={channel} onClose={onClose} />;
}

function AboutText({ channel }: { channel: Channel }) {
  const rows = [
    ['Topic', channel.topic],
    ['Description', channel.description],
  ] as const;
  return (
    <dl className="space-y-4" data-qa="channel-about">
      {rows.map(([label, value]) => (
        <div key={label}>
          <dt className="text-xs font-medium uppercase tracking-wider text-muted">{label}</dt>
          <dd className={`mt-1 text-sm whitespace-pre-wrap ${value ? 'text-fg-soft' : 'text-muted'}`}>
            {value || 'None yet'}
          </dd>
        </div>
      ))}
    </dl>
  );
}

function AboutForm({ channel, onClose }: Omit<Props, 'canModerate'>) {
  const [name, setName] = useState(channel.name ?? '');
  const [topic, setTopic] = useState(channel.topic ?? '');
  const [description, setDescription] = useState(channel.description ?? '');
  const [confirmArchive, setConfirmArchive] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const updateChannel = useUpdateChannel(channel.workspace_id, channel.id);
  const archiveChannel = useArchiveChannel(channel.workspace_id);

  const handleSave = async (e: FormEvent) => {
    e.preventDefault();
    if (!name.trim()) return;
    setError(null);
    try {
      await updateChannel.mutateAsync({
        name: name.trim(),
        topic: topic.trim(),
        description: description.trim(),
      });
      onClose();
    } catch (err: unknown) {
      setError((err as { message?: string })?.message || 'Failed to save the channel');
    }
  };

  const handleArchive = async () => {
    await archiveChannel.mutateAsync(channel.id);
    onClose();
  };

  return (
    <>
      <form onSubmit={handleSave} noValidate className="space-y-4">
        <TextInput
          label="Name"
          value={name}
          onChange={(e) => setName(e.target.value)}
          maxLength={80}
          dataQa="channel-about-name"
          required
        />
        <TextInput
          label="Topic"
          optional
          value={topic}
          onChange={(e) => setTopic(e.target.value)}
          placeholder="What is this channel about?"
          dataQa="channel-about-topic"
        />
        <TextArea
          label="Description"
          optional
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          dataQa="channel-about-description"
        />
        {error && (
          <p role="alert" className="text-sm text-danger">
            {error}
          </p>
        )}
        <div className="flex items-center justify-between gap-2 pt-2">
          <Button
            variant="ghost"
            size="sm"
            icon={<Archive className="h-3.5 w-3.5" />}
            onClick={() => setConfirmArchive(true)}
            data-qa="channel-about-archive"
            className="-ml-2.5 hover:text-danger"
          >
            Archive channel
          </Button>
          <Button
            type="submit"
            variant="primary"
            loading={updateChannel.isPending}
            disabled={!name.trim()}
            data-qa="channel-about-save"
          >
            Save
          </Button>
        </div>
      </form>
      {confirmArchive && (
        <ConfirmDialog
          title={`Archive #${channel.name ?? 'channel'}?`}
          description="It leaves the channel list and nobody can post in it, until an admin unarchives it. Its history stays readable."
          confirmLabel="Archive channel"
          onConfirm={handleArchive}
          onClose={() => setConfirmArchive(false)}
          dataQa="channel-about-archive-dialog"
        />
      )}
    </>
  );
}
