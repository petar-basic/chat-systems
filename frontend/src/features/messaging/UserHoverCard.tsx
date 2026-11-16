import { useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router';
import { MessageSquare, Moon, Phone, UserRound } from 'lucide-react';
import { HoverCard } from '@/shared/components/Popover/HoverCard';
import { Avatar } from '@/shared/components/Avatar/Avatar';
import PresenceDot from '@/shared/components/PresenceDot';
import { useUserCache } from '@/stores/users';
import { useWorkspaceStore } from '@/stores/workspace';
import { useHuddleStore } from '@/stores/huddle';
import { usePresenceStore } from '@/stores/presence';
import { useCurrentUser } from '@/features/instance';
import { FieldList, cardFields, useProfileFields } from '@/features/people';
import { useProfilePanel } from '@/stores/profilePanel';
import { useOpenConversation } from '@/features/messaging/hooks/useConversations';
import { useStartHuddle } from '@/features/huddle';
import { displayNameOf } from '@/lib/userHelpers';
import { currentPause, formatResume } from '@/lib/pause';
import { logger } from '@/lib/logger';
import { ROUTES } from '@/shared/constants';
import { ReactionEmoji } from '@/shared/components/ReactionEmoji';
import { LocalTimeLine } from './LocalTimeLine';

interface Props {
  userId: string;
  name: string;
  avatarUrl?: string | null;
  children: ReactNode;
  triggerClassName?: string;
}

export function UserHoverCard({ userId, name, avatarUrl, children, triggerClassName }: Props) {
  const currentWorkspace = useWorkspaceStore((s) => s.currentWorkspace);

  if (!currentWorkspace) return <>{children}</>;

  return (
    <HoverCard
      placement="top-start"
      dataQa="user-hover-card"
      triggerClassName={triggerClassName}
      panelClassName="w-64 rounded-xl bg-surface border border-line-strong shadow-2xl p-4"
      content={
        <UserCardBody
          userId={userId}
          name={name}
          avatarUrl={avatarUrl}
          workspaceId={currentWorkspace.id}
          instanceUrl={currentWorkspace.instanceUrl}
        />
      }
    >
      {children}
    </HoverCard>
  );
}

function UserCardBody({
  userId,
  name,
  avatarUrl,
  workspaceId,
  instanceUrl,
}: {
  userId: string;
  name: string;
  avatarUrl?: string | null;
  workspaceId: string;
  instanceUrl: string;
}) {
  const { t } = useTranslation(['messages', 'common', 'huddle']);
  const navigate = useNavigate();
  const { data: self } = useCurrentUser();
  const cached = useUserCache((s) => s.getUser(userId));
  const presence = usePresenceStore((s) => s.getStatus(userId));
  const inHuddle = useHuddleStore((s) => !!s.active);
  const openConversation = useOpenConversation(workspaceId, instanceUrl);
  const startHuddle = useStartHuddle(workspaceId, instanceUrl);
  const { fields } = useProfileFields(workspaceId, instanceUrl);
  const showProfile = useProfilePanel((s) => s.show);
  const [busy, setBusy] = useState(false);

  const isSelf = self?.id === userId;
  const left = !!cached?.left;
  const displayName = displayNameOf(cached?.display_name ?? name);
  const title = cached?.title?.trim();
  const pronouns = cached?.pronouns?.trim();
  const statusText = cached?.status_text?.trim();
  const statusEmoji = cached?.status_emoji?.trim();
  const paused = currentPause({ dnd_until: cached?.notifications_paused_until });

  const onMessage = async () => {
    if (busy) return;
    setBusy(true);
    try {
      const conversation = await openConversation.mutateAsync([userId]);
      navigate(ROUTES.conversation(workspaceId, conversation.id));
    } catch (err) {
      logger.error('UserHoverCard', 'openConversation', err);
    } finally {
      setBusy(false);
    }
  };

  const onHuddle = async () => {
    if (busy || inHuddle || !self) return;
    setBusy(true);
    try {
      const res = await startHuddle.mutateAsync({ dm_partner_id: userId });
      useHuddleStore.getState().setActive({
        huddleId: res.huddle_id,
        workspaceId,
        instanceUrl,
        selfUserId: self.id,
        scope: { kind: 'dm', partnerId: userId },
      });
    } catch (err) {
      logger.error('UserHoverCard', 'startHuddle', err);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-start gap-3">
        <Avatar userId={userId} name={displayName} avatarUrl={cached?.avatar_url ?? avatarUrl} size="xl" />
        <div className="min-w-0 flex-1 pt-0.5">
          <div className="flex items-baseline gap-1.5 min-w-0">
            <span className="text-sm font-semibold text-fg truncate">{displayName}</span>
            {pronouns && (
              <span className="text-xs text-muted shrink-0" data-qa="hover-card-pronouns">
                {pronouns}
              </span>
            )}
          </div>
          {title && (
            <div className="text-xs text-fg-dim truncate" data-qa="hover-card-title">
              {title}
            </div>
          )}
          {left ? (
            <div className="text-xs text-muted mt-0.5" data-qa="hover-card-left">
              {t('hoverCard.left')}
            </div>
          ) : (
            <div className="flex items-center gap-1.5 mt-0.5">
              <PresenceDot userId={userId} className="w-2 h-2" />
              <span className="text-xs text-muted">{t(`presence.${presence}`, { ns: 'common' })}</span>
            </div>
          )}
          {paused && (
            <div className="flex items-center gap-1 text-xs text-muted mt-1" data-qa="hover-card-paused">
              <Moon className="w-3 h-3 shrink-0" />
              {t('hoverCard.paused', { time: formatResume(paused.until) })}
            </div>
          )}
          {(statusEmoji || statusText) && (
            <div className="text-xs text-fg-dim mt-1 truncate">
              {statusEmoji && (
                <span className="mr-1">
                  <ReactionEmoji emoji={statusEmoji} />
                </span>
              )}
              {statusText}
            </div>
          )}
        </div>
      </div>

      <FieldList entries={cardFields(fields, cached?.profile_fields)} dataQa="hover-card-fields" />

      <LocalTimeLine timezone={cached?.timezone} />

      {cached?.bio && (
        <p className="text-xs text-fg-dim whitespace-pre-line line-clamp-4" data-qa="hover-card-bio">
          {cached.bio}
        </p>
      )}

      {cached?.email && <div className="text-xs text-muted truncate">{cached.email}</div>}

      {!cached?.is_bot && (
        <button
          type="button"
          onClick={() => showProfile(userId)}
          data-qa="hover-card-profile"
          className="flex items-center gap-1.5 text-xs text-accent hover:underline cursor-pointer self-start"
        >
          <UserRound className="w-3.5 h-3.5" />
          {t('hoverCard.viewProfile')}
        </button>
      )}

      {!isSelf && !left && (
        <div className="flex gap-2">
          <button
            onClick={() => void onMessage()}
            disabled={busy}
            data-qa="hover-card-message"
            className="flex-1 flex items-center justify-center gap-1.5 px-2 py-1.5 rounded-lg bg-raised hover:bg-elevated text-fg text-xs font-medium transition cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
          >
            <MessageSquare className="w-3.5 h-3.5" />
            {t('hoverCard.message')}
          </button>
          <button
            onClick={() => void onHuddle()}
            disabled={busy || inHuddle}
            data-tooltip={inHuddle ? t('busy', { ns: 'huddle' }) : t('start', { ns: 'huddle' })}
            data-qa="hover-card-huddle"
            className="flex-1 flex items-center justify-center gap-1.5 px-2 py-1.5 rounded-lg bg-raised hover:bg-elevated text-fg text-xs font-medium transition cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
          >
            <Phone className="w-3.5 h-3.5" />
            {t('hoverCard.huddle')}
          </button>
        </div>
      )}
    </div>
  );
}
