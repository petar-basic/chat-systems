import { useState, type ReactNode } from 'react';
import { useNavigate } from 'react-router';
import { MessageSquare, Phone } from 'lucide-react';
import { HoverCard } from '@/shared/components/Popover/HoverCard';
import { Avatar } from '@/shared/components/Avatar/Avatar';
import PresenceDot from '@/components/PresenceDot';
import { useUserCache } from '@/stores/users';
import { useWorkspaceStore } from '@/stores/workspace';
import { useHuddleStore } from '@/stores/huddle';
import { usePresenceStore } from '@/stores/presence';
import { useCurrentUser } from '@/hooks/queries/useAuth';
import { useOpenConversation } from '@/hooks/queries/useConversations';
import { useStartHuddle } from '@/hooks/queries/useHuddle';
import { displayNameOf } from '@/lib/userHelpers';
import { logger } from '@/lib/logger';
import { ROUTES } from '@/shared/constants';

interface Props {
  userId: string;
  name: string;
  avatarUrl?: string | null;
  children: ReactNode;
  triggerClassName?: string;
}

const PRESENCE_LABEL = { online: 'Online', away: 'Away', offline: 'Offline' } as const;

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
  const navigate = useNavigate();
  const { data: self } = useCurrentUser();
  const cached = useUserCache((s) => s.getUser(userId));
  const presence = usePresenceStore((s) => s.getStatus(userId));
  const inHuddle = useHuddleStore((s) => !!s.active);
  const openConversation = useOpenConversation(workspaceId, instanceUrl);
  const startHuddle = useStartHuddle(workspaceId, instanceUrl);
  const [busy, setBusy] = useState(false);

  const isSelf = self?.id === userId;
  const displayName = displayNameOf(cached?.display_name ?? name);
  const statusText = cached?.status_text?.trim();
  const statusEmoji = cached?.status_emoji?.trim();

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
          <div className="text-sm font-semibold text-fg truncate">{displayName}</div>
          <div className="flex items-center gap-1.5 mt-0.5">
            <PresenceDot userId={userId} className="w-2 h-2" />
            <span className="text-xs text-muted">{PRESENCE_LABEL[presence]}</span>
          </div>
          {(statusEmoji || statusText) && (
            <div className="text-xs text-fg-dim mt-1 truncate">
              {statusEmoji && <span className="mr-1">{statusEmoji}</span>}
              {statusText}
            </div>
          )}
        </div>
      </div>

      {cached?.email && <div className="text-xs text-muted truncate">{cached.email}</div>}

      {!isSelf && (
        <div className="flex gap-2">
          <button
            onClick={() => void onMessage()}
            disabled={busy}
            data-qa="hover-card-message"
            className="flex-1 flex items-center justify-center gap-1.5 px-2 py-1.5 rounded-lg bg-raised hover:bg-elevated text-fg text-xs font-medium transition cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
          >
            <MessageSquare className="w-3.5 h-3.5" />
            Message
          </button>
          <button
            onClick={() => void onHuddle()}
            disabled={busy || inHuddle}
            title={inHuddle ? 'Already in a huddle' : 'Start huddle'}
            data-qa="hover-card-huddle"
            className="flex-1 flex items-center justify-center gap-1.5 px-2 py-1.5 rounded-lg bg-raised hover:bg-elevated text-fg text-xs font-medium transition cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
          >
            <Phone className="w-3.5 h-3.5" />
            Huddle
          </button>
        </div>
      )}
    </div>
  );
}
