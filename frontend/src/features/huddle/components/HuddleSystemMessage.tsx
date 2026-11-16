import { MessageSquare, Phone } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Trans } from '@/shared/components/Trans/Trans';
import { useWorkspaceStore, type Message } from '@/stores/workspace';
import { useHuddleStore } from '@/stores/huddle';
import { useUserCache } from '@/stores/users';
import { displayNameOf } from '@/lib/userHelpers';

const NAMED = 3;

interface Props {
  message: Message;
  onThreadOpen?: (message: Message) => void;
}

export function HuddleSystemMessage({ message, onThreadOpen }: Props) {
  const { t, i18n } = useTranslation('huddle');
  const { t: tm } = useTranslation('messages');
  const { getUser } = useUserCache();
  const channelId = message.channel_id;
  const huddleId = message.metadata?.huddle_id ?? '';
  const name = displayNameOf(getUser(message.metadata?.initiator_id ?? message.user_id)?.display_name);
  const channelHuddle = useWorkspaceStore((s) => s.activeHuddleChannels.get(channelId));
  const currentWorkspace = useWorkspaceStore((s) => s.currentWorkspace);
  const currentUserId = useWorkspaceStore((s) => s.currentUserId);
  const active = useHuddleStore((s) => s.active);

  const endedAt = message.metadata?.ended_at;
  const stillActive = !endedAt && channelHuddle?.huddleId === huddleId;
  const inThisHuddle = active?.scope.kind === 'channel' && active.scope.channelId === channelId;
  const canJoin = stillActive && !inThisHuddle && !active && !!currentWorkspace && !!currentUserId;

  const join = () => {
    if (!canJoin) return;
    useHuddleStore.getState().setActive({
      huddleId,
      workspaceId: currentWorkspace.id,
      instanceUrl: currentWorkspace.instanceUrl,
      selfUserId: currentUserId,
      scope: { kind: 'channel', channelId },
    });
  };

  const lasted = () => {
    if (!endedAt) return '';
    const minutes = Math.round((Date.parse(endedAt) - Date.parse(message.created_at)) / 60_000);
    if (minutes < 1) return t('system.underMinute');
    if (minutes < 60) return t('system.minutes', { count: minutes });
    return t('system.hours', { hours: Math.floor(minutes / 60), minutes: minutes % 60 });
  };

  const people = () => {
    const ids = message.metadata?.participant_ids ?? [];
    const names = ids.slice(0, NAMED).map((id) => displayNameOf(getUser(id)?.display_name));
    if (ids.length > NAMED) names.push(t('system.others', { count: ids.length - NAMED }));
    return new Intl.ListFormat(i18n.language, { type: 'conjunction' }).format(names);
  };

  return (
    <div className="py-1.5 px-2 text-sm text-muted" data-qa="huddle-system-message">
      <div className="flex items-center gap-2">
        <Phone className="w-3.5 h-3.5 text-accent shrink-0" />
        <span>
          <Trans
            t={t}
            i18nKey="system.started"
            values={{ name }}
            components={{ name: <span className="text-fg-soft font-medium" /> }}
          />
        </span>
        {canJoin && (
          <button
            onClick={join}
            className="ml-1 px-2 py-0.5 rounded-md bg-green-600/20 text-success hover:bg-green-600/30 text-xs font-medium transition"
          >
            {t('system.join')}
          </button>
        )}
      </div>
      {endedAt && (
        <p data-qa="huddle-summary" className="ml-5.5 mt-0.5 text-xs">
          {t('system.lasted', { duration: lasted() })}
          {(message.metadata?.participant_ids?.length ?? 0) > 0 &&
            ` · ${t('system.with', { names: people() })}`}
        </p>
      )}
      {message.reply_count > 0 && onThreadOpen && (
        <button
          onClick={() => onThreadOpen(message)}
          data-qa="message-thread-open"
          className="ml-5.5 mt-1 flex items-center gap-1.5 text-xs text-accent hover:text-accent-soft transition group/thread"
        >
          <MessageSquare className="w-3.5 h-3.5" />
          <span className="font-medium">{tm('item.replies', { count: message.reply_count })}</span>
          <span className="text-muted group-hover/thread:text-accent transition">
            {tm('item.viewThread')}
          </span>
        </button>
      )}
    </div>
  );
}
