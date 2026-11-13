import { useState, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/shared/components/Button/Button';
import { Modal } from '@/shared/components/Modal/Modal';
import { TextInput } from '@/shared/components/Form/TextInput';
import { CHANNEL_NAME_MAX, channelNameFrom } from '@/features/channel';
import type { Channel } from '@/stores/workspace';
import { useConvertConversation } from './hooks/useConversations';

interface Props {
  conversationId: string;
  instanceUrl: string;
  channels: Channel[];
  onConverted: (channel: Channel) => void;
  onClose: () => void;
}

export function ConvertConversationModal({
  conversationId,
  instanceUrl,
  channels,
  onConverted,
  onClose,
}: Props) {
  const { t } = useTranslation(['messages', 'sidebar', 'common']);
  const [name, setName] = useState('');
  const [error, setError] = useState<string | null>(null);
  const convert = useConvertConversation(instanceUrl);

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    const trimmed = name.replace(/^-+|-+$/g, '');
    if (!trimmed) {
      setError(t('newChannel.nameRequired', { ns: 'sidebar' }));
      return;
    }
    if (channels.some((ch) => ch.name?.toLowerCase() === trimmed.toLowerCase())) {
      setError(t('newChannel.nameTaken', { ns: 'sidebar' }));
      return;
    }
    setError(null);
    try {
      const channel = await convert.mutateAsync({ conversationId, name: trimmed });
      onConverted(channel);
    } catch (err) {
      setError(err instanceof Error ? err.message : t('conversation.convert.failed'));
    }
  };

  return (
    <Modal
      title={t('conversation.convert.title')}
      description={t('conversation.convert.description')}
      onClose={onClose}
      dataQa="conversation-convert-modal"
      onSubmit={handleSubmit}
      dismissLabel={t('cancel', { ns: 'common' })}
      actions={
        <Button
          type="submit"
          variant="primary"
          loading={convert.isPending}
          data-qa="conversation-convert-submit"
        >
          {t('conversation.convert.submit')}
        </Button>
      }
    >
      <TextInput
        label={t('newChannel.name', { ns: 'sidebar' })}
        value={name}
        onChange={(e) => {
          setName(channelNameFrom(e.target.value));
          if (error) setError(null);
        }}
        leading={<span className="text-sm">#</span>}
        placeholder={t('newChannel.namePlaceholder', { ns: 'sidebar' })}
        maxLength={CHANNEL_NAME_MAX}
        counter
        error={error}
        autoComplete="off"
        dataQa="conversation-convert-name"
        data-autofocus
      />
    </Modal>
  );
}
