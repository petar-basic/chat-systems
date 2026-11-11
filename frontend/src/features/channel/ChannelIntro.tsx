import { Bookmark, Hash, Lock, UserPlus } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';
import { Button } from '@/shared/components/Button/Button';
import type { Channel } from '@/stores/workspace';

interface Props {
  channel: Channel;
  onAddPeople?: () => void;
  onAddBookmark?: () => void;
}

function purposeOf(channel: Channel, t: TFunction<'channel'>) {
  const own = channel.description?.trim() || channel.topic?.trim();
  if (own) return own;
  if (channel.is_default) {
    return t('intro.defaultPurpose');
  }
  return t('intro.noPurpose');
}

export function ChannelIntro({ channel, onAddPeople, onAddBookmark }: Props) {
  const { t } = useTranslation('channel');
  const Icon = channel.channel_type === 'private' ? Lock : Hash;

  return (
    <div className="max-w-lg text-left px-6" data-qa="channel-intro">
      <div className="w-12 h-12 rounded-xl bg-raised flex items-center justify-center mb-4">
        <Icon className="w-6 h-6 text-fg-dim" aria-hidden />
      </div>
      <h2 className="text-xl font-bold text-fg">{t('intro.title', { name: channel.name })}</h2>
      <p className="mt-2 text-sm text-muted" data-qa="channel-intro-purpose">
        {purposeOf(channel, t)}
      </p>
      {(onAddPeople || onAddBookmark) && (
        <div className="mt-4 flex flex-wrap gap-2">
          {onAddPeople && (
            <Button
              size="sm"
              icon={<UserPlus className="w-4 h-4" />}
              onClick={onAddPeople}
              data-qa="channel-intro-add-people"
            >
              {t('intro.addPeople')}
            </Button>
          )}
          {onAddBookmark && (
            <Button
              size="sm"
              icon={<Bookmark className="w-4 h-4" />}
              onClick={onAddBookmark}
              data-qa="channel-intro-add-bookmark"
            >
              {t('intro.addBookmark')}
            </Button>
          )}
        </div>
      )}
    </div>
  );
}
