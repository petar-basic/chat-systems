import { Hash } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Trans } from '@/shared/components/Trans/Trans';
import { Button } from '@/shared/components/Button/Button';
import { toast } from '@/shared/components/Toast';
import { toUserMessage } from '@/lib/errors';
import type { Channel } from '@/stores/workspace';
import { useJoinChannel } from './hooks/useChannels';

interface Props {
  channel: Channel;
  instanceUrl?: string;
}

export function ChannelPreviewBar({ channel, instanceUrl }: Props) {
  const { t } = useTranslation('channel');
  const join = useJoinChannel(channel.workspace_id, instanceUrl);
  const onJoin = () =>
    join.mutate(channel.id, {
      onError: (err) => toast.error(toUserMessage(err, t('preview.joinFailed'))),
    });

  return (
    <div
      data-qa="channel-preview"
      className="mx-4 mb-4 px-4 py-3 rounded-lg bg-surface/60 border border-line text-sm text-muted flex items-center gap-3"
    >
      <Hash className="w-4 h-4 shrink-0" />
      <span className="flex-1 min-w-0">
        <Trans
          t={t}
          i18nKey="preview.text"
          values={{ name: channel.name }}
          components={{ name: <span className="font-semibold text-fg-soft" /> }}
        />
      </span>
      <Button
        variant="primary"
        size="sm"
        loading={join.isPending}
        onClick={onJoin}
        data-qa="channel-preview-join"
      >
        {t('preview.join')}
      </Button>
    </div>
  );
}
