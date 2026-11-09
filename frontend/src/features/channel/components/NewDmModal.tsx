import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/shared/components/Button/Button';
import { Check } from 'lucide-react';
import { Modal } from '@/shared/components/Modal/Modal';
import { TextInput } from '@/shared/components/Form/TextInput';
import { Avatar } from '@/shared/components/Avatar/Avatar';
import type { WorkspaceMember } from '@/stores/workspace';
import { matchesPerson } from '@/lib/userHelpers';

const MAX_GROUP_OTHERS = 8;

interface Props {
  members: WorkspaceMember[];
  currentUserId: string | undefined;
  onStart: (participantIds: string[]) => void;
  onClose: () => void;
}

export function NewDmModal({ members, currentUserId, onStart, onClose }: Props) {
  const { t } = useTranslation(['sidebar', 'common']);
  const [search, setSearch] = useState('');
  const [selected, setSelected] = useState<string[]>([]);

  const togglePerson = (userId: string) =>
    setSelected((picked) =>
      picked.includes(userId)
        ? picked.filter((id) => id !== userId)
        : picked.length >= MAX_GROUP_OTHERS
          ? picked
          : [...picked, userId],
    );

  const candidates = members.filter((m) => m.user_id !== currentUserId && matchesPerson(m, search));

  return (
    <Modal
      title={t('newDm.title')}
      description={t('newDm.description')}
      onClose={onClose}
      size="sm"
      dataQa="new-dm-modal"
      footerStart={
        <span className="text-xs text-muted" data-qa="new-dm-selected-count">
          {selected.length === 0 ? t('newDm.nobody') : t('newDm.selected', { count: selected.length })}
        </span>
      }
      dismissLabel={t('cancel', { ns: 'common' })}
      actions={
        <Button
          variant="primary"
          onClick={() => {
            const people = selected;
            onClose();
            onStart(people);
          }}
          disabled={selected.length === 0}
          data-qa="new-dm-start"
        >
          {selected.length > 1 ? t('newDm.startGroup') : t('newDm.start')}
        </Button>
      }
    >
      <TextInput
        label={t('newDm.search')}
        labelHidden
        value={search}
        onChange={(e) => setSearch(e.target.value)}
        placeholder={t('newDm.searchPlaceholder')}
        dataQa="new-dm-search"
        className="mb-3"
      />
      <div className="flex flex-col gap-1">
        {candidates.length === 0 ? (
          <div className="px-3 py-4 text-sm text-muted text-center">{t('newDm.noPeople')}</div>
        ) : (
          candidates.map((m) => {
            const picked = selected.includes(m.user_id);
            return (
              <button
                key={m.user_id}
                onClick={() => togglePerson(m.user_id)}
                aria-pressed={picked}
                data-qa="new-dm-candidate"
                data-user-id={m.user_id}
                className={`w-full px-3 py-2 flex items-center gap-3 rounded-lg text-left transition ${
                  picked ? 'bg-purple-600/20 text-fg' : 'hover:bg-raised/50'
                }`}
              >
                <Avatar userId={m.user_id} name={m.display_name || m.email} avatarUrl={m.avatar_url} />
                <div className="min-w-0">
                  <div className="flex items-baseline gap-1.5">
                    <span className="text-sm font-medium truncate">{m.display_name || m.email}</span>
                    {m.pronouns && <span className="text-xs text-muted shrink-0">{m.pronouns}</span>}
                  </div>
                  <div className="text-xs text-muted truncate">{m.title || m.email}</div>
                </div>
                {picked && <Check className="w-4 h-4 text-accent-soft ml-auto shrink-0" />}
              </button>
            );
          })
        )}
      </div>
    </Modal>
  );
}
