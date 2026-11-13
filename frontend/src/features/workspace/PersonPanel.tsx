import { useState } from 'react';
import { useNavigate } from 'react-router';
import { useTranslation } from 'react-i18next';
import { MessageSquare, Pencil, UserRound } from 'lucide-react';
import { PanelHeader } from '@/shared/components/PanelHeader/PanelHeader';
import { Avatar } from '@/shared/components/Avatar/Avatar';
import { Button } from '@/shared/components/Button/Button';
import PresenceDot from '@/shared/components/PresenceDot';
import { ReactionEmoji } from '@/shared/components/ReactionEmoji';
import { useUserCache } from '@/stores/users';
import { usePresenceStore } from '@/stores/presence';
import { useCurrentUser } from '@/features/instance';
import { LocalTimeLine, useOpenConversation } from '@/features/messaging';
import { FieldList, filledFields, useProfileFields } from '@/features/people';
import { displayNameOf } from '@/lib/userHelpers';
import { logger } from '@/lib/logger';
import { ROUTES } from '@/shared/constants';
import { useCurrentWorkspaceRole } from './hooks/useCurrentWorkspaceRole';
import { ProfileValuesDialog } from './ProfileValuesDialog';
import { isWorkspaceAdmin } from './settings/sections';

interface Props {
  userId: string;
  workspaceId: string;
  instanceUrl?: string;
  onClose: () => void;
}

export function PersonPanel({ userId, workspaceId, instanceUrl, onClose }: Props) {
  const { t } = useTranslation(['profile', 'messages', 'common']);
  const navigate = useNavigate();
  const { data: self } = useCurrentUser();
  const { role } = useCurrentWorkspaceRole();
  const isAdmin = isWorkspaceAdmin(role);
  const person = useUserCache((s) => s.getUser(userId));
  const presence = usePresenceStore((s) => s.getStatus(userId));
  const { fields } = useProfileFields(workspaceId, instanceUrl);
  const openConversation = useOpenConversation(workspaceId, instanceUrl);
  const [editing, setEditing] = useState(false);

  const name = displayNameOf(person?.display_name || person?.email);
  const left = !!person?.left;
  const isSelf = self?.id === userId;

  const onMessage = async () => {
    try {
      const conversation = await openConversation.mutateAsync([userId]);
      navigate(ROUTES.conversation(workspaceId, conversation.id));
    } catch (err) {
      logger.error('PersonPanel', 'openConversation', err);
    }
  };

  return (
    <div
      className="w-full lg:w-80 max-lg:fixed max-lg:inset-0 max-lg:z-40 bg-surface border-l border-line/50 flex flex-col"
      data-qa="person-panel"
    >
      <PanelHeader
        title={t('panel.title')}
        icon={<UserRound className="h-4 w-4 text-muted" />}
        onClose={onClose}
      />
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        <div className="flex flex-col items-center text-center gap-2">
          <Avatar userId={userId} name={name} avatarUrl={person?.avatar_url} size="2xl" />
          <div className="min-w-0">
            <div className="flex items-baseline justify-center gap-1.5">
              <h3 className="text-lg font-semibold text-fg truncate" data-qa="person-panel-name">
                {name}
              </h3>
              {person?.pronouns && <span className="text-xs text-muted shrink-0">{person.pronouns}</span>}
            </div>
            {person?.title && (
              <div className="text-sm text-fg-dim" data-qa="person-panel-title">
                {person.title}
              </div>
            )}
          </div>
        </div>

        <FieldList
          entries={filledFields(fields, person?.profile_fields)}
          dataQa="person-panel-fields"
          className="text-sm"
        />

        {left ? (
          <p className="text-xs text-muted">{t('hoverCard.left', { ns: 'messages' })}</p>
        ) : (
          <div className="flex items-center gap-1.5">
            <PresenceDot userId={userId} className="w-2 h-2" />
            <span className="text-xs text-muted">{t(`presence.${presence}`, { ns: 'common' })}</span>
          </div>
        )}
        {(person?.status_emoji || person?.status_text) && (
          <div className="text-sm text-fg-dim">
            {person.status_emoji && (
              <span className="mr-1">
                <ReactionEmoji emoji={person.status_emoji} />
              </span>
            )}
            {person.status_text}
          </div>
        )}
        <LocalTimeLine timezone={person?.timezone} />
        {person?.email && <div className="text-xs text-muted break-all">{person.email}</div>}
        {person?.bio && <p className="text-sm text-fg-dim whitespace-pre-line">{person.bio}</p>}

        <div className="flex flex-wrap gap-2">
          {!isSelf && !left && (
            <Button
              size="sm"
              icon={<MessageSquare className="w-3.5 h-3.5" />}
              onClick={() => void onMessage()}
              loading={openConversation.isPending}
              data-qa="person-panel-message"
            >
              {t('hoverCard.message', { ns: 'messages' })}
            </Button>
          )}
          {isAdmin && !left && !person?.is_bot && fields.length > 0 && (
            <Button
              size="sm"
              icon={<Pencil className="w-3.5 h-3.5" />}
              onClick={() => setEditing(true)}
              data-qa="person-panel-edit-fields"
            >
              {t('panel.editFields')}
            </Button>
          )}
        </div>
      </div>

      {editing && (
        <ProfileValuesDialog
          userId={userId}
          name={name}
          workspaceId={workspaceId}
          instanceUrl={instanceUrl}
          fields={fields}
          values={person?.profile_fields ?? {}}
          onClose={() => setEditing(false)}
        />
      )}
    </div>
  );
}
