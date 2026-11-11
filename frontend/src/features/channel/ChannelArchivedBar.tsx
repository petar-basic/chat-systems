import { Archive } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Trans } from '@/shared/components/Trans/Trans';
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
  const { t } = useTranslation('channel');
  const unarchive = useUnarchiveChannel(channel.workspace_id);
  const onUnarchive = () =>
    unarchive.mutate(channel.id, {
      onError: (err) => toast.error(toUserMessage(err, t('archived.unarchiveFailed'))),
    });

  return (
    <div
      data-qa="channel-archived"
      className="mx-4 mb-4 px-4 py-3 rounded-lg bg-surface/60 border border-line text-sm text-muted flex items-center gap-3"
    >
      <Archive className="w-4 h-4 shrink-0" />
      <span className="flex-1 min-w-0">
        <Trans
          t={t}
          i18nKey="archived.text"
          values={{ name: channel.name }}
          components={{ name: <span className="font-semibold text-fg-soft" /> }}
        />
      </span>
      {canUnarchive && (
        <Button
          variant="primary"
          size="sm"
          loading={unarchive.isPending}
          onClick={onUnarchive}
          data-qa="channel-unarchive"
        >
          {t('archived.unarchive')}
        </Button>
      )}
    </div>
  );
}
