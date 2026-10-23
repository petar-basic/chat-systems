import { useState, type FormEvent } from 'react';
import { Button } from '@/shared/components/Button/Button';
import { Modal } from '@/shared/components/Modal/Modal';
import { TextInput } from '@/shared/components/Form/TextInput';
import { TextArea } from '@/shared/components/Form/TextArea';
import { Checkbox } from '@/shared/components/Form/Checkbox';
import { RadioCards } from '@/shared/components/Form/RadioCards';
import { Hash, Lock } from 'lucide-react';
import { CHANNEL_NAME_MAX, channelNameFrom } from '../channelName';
import type { CreateChannelDraft } from '@/models/channel';
import type { Channel } from '@/stores/workspace';

interface Props {
  channels: Channel[];
  onCreate: (draft: CreateChannelDraft) => Promise<void>;
  onClose: () => void;
}

export function CreateChannelModal({ channels, onCreate, onClose }: Props) {
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [isPrivate, setIsPrivate] = useState(false);
  const [announcementOnly, setAnnouncementOnly] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    const trimmed = name.replace(/^-+|-+$/g, '');
    if (!trimmed) {
      setError('Enter a channel name.');
      return;
    }
    if (channels.some((ch) => ch.name?.toLowerCase() === trimmed.toLowerCase())) {
      setError('A channel with that name already exists.');
      return;
    }
    setError(null);
    setCreating(true);
    try {
      await onCreate({
        name: trimmed,
        description: description.trim() || undefined,
        isPrivate,
        announcementOnly,
      });
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Could not create the channel.');
    } finally {
      setCreating(false);
    }
  };

  return (
    <Modal
      title="Create channel"
      description="Channels are where a team talks about one topic. Pick a name people will recognise."
      onClose={onClose}
      dataQa="create-channel-modal"
      onSubmit={handleSubmit}
      dismissLabel="Cancel"
      actions={
        <Button type="submit" variant="primary" loading={creating} data-qa="create-channel-submit">
          Create
        </Button>
      }
    >
      <div className="space-y-5">
        <TextInput
          label="Name"
          value={name}
          onChange={(e) => {
            setName(channelNameFrom(e.target.value));
            if (error) setError(null);
          }}
          leading={<span className="text-sm">#</span>}
          placeholder="e.g. plan-budget"
          maxLength={CHANNEL_NAME_MAX}
          counter
          error={error}
          autoComplete="off"
          dataQa="create-channel-name"
          data-autofocus
        />

        <RadioCards
          label="Visibility"
          value={isPrivate ? 'private' : 'public'}
          onChange={(value) => setIsPrivate(value === 'private')}
          options={[
            {
              value: 'public',
              label: 'Public',
              description: 'Anyone in the workspace can find and join it.',
              icon: <Hash className="w-4 h-4" />,
              dataQa: 'create-channel-public',
            },
            {
              value: 'private',
              label: 'Private',
              description: 'Only people who are invited can find or read it.',
              icon: <Lock className="w-4 h-4" />,
              dataQa: 'create-channel-private',
            },
          ]}
        />

        <TextArea
          label="Description"
          optional
          rows={2}
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          placeholder="What is this channel for?"
          dataQa="create-channel-description"
        />

        <Checkbox
          label="Announcement channel"
          description="Only admins can post. Everyone else can still read and react."
          checked={announcementOnly}
          onChange={setAnnouncementOnly}
          dataQa="create-channel-announcement"
        />
      </div>
    </Modal>
  );
}
