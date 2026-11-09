import { useState, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/shared/components/Button/Button';
import { Modal } from '@/shared/components/Modal/Modal';
import { TextInput } from '@/shared/components/Form/TextInput';
import { TextArea } from '@/shared/components/Form/TextArea';
import { Checkbox } from '@/shared/components/Form/Checkbox';
import { RadioCards } from '@/shared/components/Form/RadioCards';
import { Hash, Lock } from 'lucide-react';
import { CHANNEL_NAME_MAX, channelNameFrom } from '../channelName';
import type { CreateChannelDraft } from '@/shared/models/channel';
import type { Channel } from '@/stores/workspace';
import type { ChannelKind } from '@/lib/channelPermissions';

interface Props {
  kinds: ChannelKind[];
  channels: Channel[];
  onCreate: (draft: CreateChannelDraft) => Promise<void>;
  onClose: () => void;
}

export function CreateChannelModal({ kinds, channels, onCreate, onClose }: Props) {
  const { t } = useTranslation(['sidebar', 'common']);
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [isPrivate, setIsPrivate] = useState(!kinds.includes('public'));
  const [announcementOnly, setAnnouncementOnly] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    const trimmed = name.replace(/^-+|-+$/g, '');
    if (!trimmed) {
      setError(t('newChannel.nameRequired'));
      return;
    }
    if (channels.some((ch) => ch.name?.toLowerCase() === trimmed.toLowerCase())) {
      setError(t('newChannel.nameTaken'));
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
      setError(err instanceof Error ? err.message : t('newChannel.failed'));
    } finally {
      setCreating(false);
    }
  };

  return (
    <Modal
      title={t('newChannel.title')}
      description={t('newChannel.description')}
      onClose={onClose}
      dataQa="create-channel-modal"
      onSubmit={handleSubmit}
      dismissLabel={t('cancel', { ns: 'common' })}
      actions={
        <Button type="submit" variant="primary" loading={creating} data-qa="create-channel-submit">
          {t('newChannel.create')}
        </Button>
      }
    >
      <div className="space-y-5">
        <TextInput
          label={t('newChannel.name')}
          value={name}
          onChange={(e) => {
            setName(channelNameFrom(e.target.value));
            if (error) setError(null);
          }}
          leading={<span className="text-sm">#</span>}
          placeholder={t('newChannel.namePlaceholder')}
          maxLength={CHANNEL_NAME_MAX}
          counter
          error={error}
          autoComplete="off"
          dataQa="create-channel-name"
          data-autofocus
        />

        <RadioCards
          label={t('newChannel.visibility')}
          value={isPrivate ? 'private' : 'public'}
          onChange={(value) => setIsPrivate(value === 'private')}
          columns={kinds.length > 1 ? 2 : 1}
          options={[
            {
              value: 'public' as const,
              label: t('newChannel.public'),
              description: t('newChannel.publicHint'),
              icon: <Hash className="w-4 h-4" />,
              dataQa: 'create-channel-public',
            },
            {
              value: 'private' as const,
              label: t('newChannel.private'),
              description: t('newChannel.privateHint'),
              icon: <Lock className="w-4 h-4" />,
              dataQa: 'create-channel-private',
            },
          ].filter((option) => kinds.includes(option.value))}
        />

        <TextArea
          label={t('newChannel.about')}
          optional
          rows={2}
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          placeholder={t('newChannel.aboutPlaceholder')}
          dataQa="create-channel-description"
        />

        <Checkbox
          label={t('newChannel.announcement')}
          description={t('newChannel.announcementHint')}
          checked={announcementOnly}
          onChange={setAnnouncementOnly}
          dataQa="create-channel-announcement"
        />
      </div>
    </Modal>
  );
}
