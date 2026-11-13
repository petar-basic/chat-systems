import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Globe, Lock } from 'lucide-react';
import { Button } from '@/shared/components/Button/Button';
import { Checkbox } from '@/shared/components/Form/Checkbox';
import { ConfirmDialog } from '@/shared/components/ConfirmDialog/ConfirmDialog';
import type { Channel } from '@/stores/workspace';
import { useChannelPrivacy, useSetChannelPrivacy } from '../hooks/useChannels';

interface Props {
  channel: Channel;
}

export function ChannelPrivacyField({ channel }: Props) {
  const { t } = useTranslation('channel');
  const [confirming, setConfirming] = useState(false);
  const isPrivate = channel.channel_type === 'private';
  const Icon = isPrivate ? Lock : Globe;

  return (
    <div data-qa="channel-settings-privacy">
      <p className="text-sm font-medium text-fg">{t('details.privacy.label')}</p>
      <div className="mt-1 flex items-start justify-between gap-3">
        <p className="flex items-start gap-2 text-sm text-fg-soft">
          <Icon className="mt-0.5 h-3.5 w-3.5 shrink-0" aria-hidden />
          {isPrivate ? t('details.privacy.private') : t('details.privacy.public')}
        </p>
        <Button
          variant="secondary"
          size="sm"
          onClick={() => setConfirming(true)}
          data-qa="channel-settings-privacy-change"
        >
          {isPrivate ? t('details.privacy.makePublic') : t('details.privacy.makePrivate')}
        </Button>
      </div>
      {confirming &&
        (isPrivate ? (
          <MakePublicDialog channel={channel} onClose={() => setConfirming(false)} />
        ) : (
          <MakePrivateDialog channel={channel} onClose={() => setConfirming(false)} />
        ))}
    </div>
  );
}

function MakePrivateDialog({ channel, onClose }: { channel: Channel; onClose: () => void }) {
  const { t } = useTranslation('channel');
  const setPrivacy = useSetChannelPrivacy(channel.id);
  return (
    <ConfirmDialog
      title={t('details.privacy.privateTitle', { name: channel.name ?? t('details.fallbackName') })}
      description={t('details.privacy.privateDescription')}
      confirmLabel={t('details.privacy.makePrivate')}
      tone="primary"
      onConfirm={async () => {
        await setPrivacy.mutateAsync(true);
      }}
      onClose={onClose}
      dataQa="channel-privacy-private-dialog"
    />
  );
}

function MakePublicDialog({ channel, onClose }: { channel: Channel; onClose: () => void }) {
  const { t } = useTranslation('channel');
  const [understood, setUnderstood] = useState(false);
  const contents = useChannelPrivacy(channel.id, true);
  const setPrivacy = useSetChannelPrivacy(channel.id);
  const description = contents.data
    ? t('details.privacy.publicDescription', {
        messages: t('details.privacy.messages', { count: contents.data.messages }),
        files: t('details.privacy.files', { count: contents.data.files }),
      })
    : t('details.privacy.counting');
  return (
    <ConfirmDialog
      title={t('details.privacy.publicTitle', { name: channel.name ?? t('details.fallbackName') })}
      description={description}
      confirmLabel={t('details.privacy.makePublic')}
      confirmDisabled={!understood || !contents.data}
      onConfirm={async () => {
        await setPrivacy.mutateAsync(false);
      }}
      onClose={onClose}
      dataQa="channel-privacy-public-dialog"
    >
      <Checkbox
        label={t('details.privacy.understand')}
        checked={understood}
        onChange={setUnderstood}
        dataQa="channel-privacy-public-understand"
      />
    </ConfirmDialog>
  );
}
