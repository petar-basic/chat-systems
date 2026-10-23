import { useState } from 'react';
import { Button } from '@/shared/components/Button/Button';
import { Check } from 'lucide-react';
import { Modal } from '@/shared/components/Modal/Modal';
import { TextInput } from '@/shared/components/Form/TextInput';
import { Avatar } from '@/shared/components/Avatar/Avatar';
import type { WorkspaceMember } from '@/stores/workspace';

const MAX_GROUP_OTHERS = 8;

interface Props {
  members: WorkspaceMember[];
  currentUserId: string | undefined;
  onStart: (participantIds: string[]) => void;
  onClose: () => void;
}

export function NewDmModal({ members, currentUserId, onStart, onClose }: Props) {
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

  const query = search.trim().toLowerCase();
  const candidates = members.filter(
    (m) => m.user_id !== currentUserId && (m.display_name || m.email).toLowerCase().includes(query),
  );

  return (
    <Modal
      title="New message"
      description="Pick one person for a direct message, or up to eight for a group."
      onClose={onClose}
      size="sm"
      dataQa="new-dm-modal"
      footerStart={
        <span className="text-xs text-muted" data-qa="new-dm-selected-count">
          {selected.length === 0 ? 'Nobody picked yet' : `${selected.length} selected`}
        </span>
      }
      dismissLabel="Cancel"
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
          {selected.length > 1 ? 'Start group' : 'Start chat'}
        </Button>
      }
    >
      <TextInput
        label="Search people"
        labelHidden
        value={search}
        onChange={(e) => setSearch(e.target.value)}
        placeholder="Search people…"
        dataQa="new-dm-search"
        className="mb-3"
      />
      <div className="flex flex-col gap-1">
        {candidates.length === 0 ? (
          <div className="px-3 py-4 text-sm text-muted text-center">No people found</div>
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
                  <div className="text-sm font-medium truncate">{m.display_name || m.email}</div>
                  <div className="text-xs text-muted truncate">{m.email}</div>
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
