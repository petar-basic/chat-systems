import { useState, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/shared/components/Button/Button';
import { Checkbox } from '@/shared/components/Form/Checkbox';
import type { Channel } from '@/stores/workspace';
import { useChannelRetention, useSetChannelRetention } from '@/features/workspace';
import { useUpdateChannel } from '../hooks/useChannels';
import { ChannelRetentionField } from './ChannelRetentionField';
import { choiceOf, periodOf, type PeriodChoice } from './channelPeriod';

interface Props {
  channel: Channel;
  onClose: () => void;
}

export function SettingsSection({ channel, onClose }: Props) {
  const { t } = useTranslation('channel');
  const [announcementOnly, setAnnouncementOnly] = useState(channel.settings?.post_policy === 'moderators');
  const savedBroadcastMentions = channel.settings?.allow_broadcast_mentions !== false;
  const [broadcastMentions, setBroadcastMentions] = useState(savedBroadcastMentions);
  const [retentionChoice, setRetentionChoice] = useState<PeriodChoice | null>(null);
  const [error, setError] = useState<string | null>(null);

  const updateChannel = useUpdateChannel(channel.workspace_id, channel.id);
  const retention = useChannelRetention(channel.id);
  const setRetention = useSetChannelRetention(channel.id);

  const savedChoice = retention.data ? choiceOf(retention.data) : null;
  const choice = retentionChoice ?? savedChoice;
  const period = choice ? periodOf(choice) : undefined;
  const retentionChanged =
    !!retention.data?.overrides_allowed && savedChoice !== null && period !== periodOf(savedChoice);

  const handleSave = async (e: FormEvent) => {
    e.preventDefault();
    if (period === 'invalid') return;
    setError(null);
    try {
      await updateChannel.mutateAsync({
        post_policy: announcementOnly ? 'moderators' : 'everyone',
        ...(broadcastMentions !== savedBroadcastMentions && { allow_broadcast_mentions: broadcastMentions }),
      });
      if (retentionChanged && period !== undefined) await setRetention.mutateAsync(period);
      onClose();
    } catch (err: unknown) {
      setError((err as { message?: string })?.message || t('details.saveFailed'));
    }
  };

  return (
    <form onSubmit={handleSave} noValidate className="space-y-4">
      <Checkbox
        label={t('details.settings.announcement')}
        description={t('details.settings.announcementHint')}
        checked={announcementOnly}
        onChange={setAnnouncementOnly}
        dataQa="channel-settings-announcement"
      />
      <Checkbox
        label={t('details.settings.broadcast')}
        description={t('details.settings.broadcastHint')}
        checked={broadcastMentions}
        onChange={setBroadcastMentions}
        dataQa="channel-settings-broadcast-mentions"
      />
      {retention.data && choice && (
        <ChannelRetentionField retention={retention.data} choice={choice} onChange={setRetentionChoice} />
      )}
      {error && (
        <p role="alert" className="text-sm text-danger">
          {error}
        </p>
      )}
      <div className="flex justify-end pt-2">
        <Button
          type="submit"
          variant="primary"
          loading={updateChannel.isPending || setRetention.isPending}
          disabled={period === 'invalid'}
          data-qa="channel-settings-save"
        >
          {t('details.save')}
        </Button>
      </div>
    </form>
  );
}
