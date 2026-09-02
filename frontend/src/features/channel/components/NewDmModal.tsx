import { useState } from 'react';
import { Check } from 'lucide-react';
import { Modal } from '@/shared/components/Modal/Modal';
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
    <Modal title="New Message" onClose={onClose} dataQa="new-dm-modal">
      <h2 className="text-lg font-bold mb-1">New message</h2>
      <p className="text-xs text-muted mb-3">
        Pick one person for a direct message, or up to eight for a group.
      </p>
      <input
        type="text"
        value={search}
        onChange={(e) => setSearch(e.target.value)}
        placeholder="Search people…"
        aria-label="Search people"
        data-qa="new-dm-search"
        className="w-full px-3 py-2 mb-3 bg-raised/50 border border-line-strong rounded-lg text-sm text-fg placeholder-muted focus:outline-none focus:ring-2 focus:ring-purple-500"
      />
      <div className="flex flex-col gap-1 max-h-64 overflow-y-auto">
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
      <div className="mt-4 flex items-center justify-between gap-2">
        <span className="text-xs text-muted" data-qa="new-dm-selected-count">
          {selected.length === 0 ? 'Nobody picked yet' : `${selected.length} selected`}
        </span>
        <div className="flex gap-2">
          <button onClick={onClose} className="px-4 py-2 text-muted hover:text-fg transition">
            Cancel
          </button>
          <button
            onClick={() => {
              const people = selected;
              onClose();
              onStart(people);
            }}
            disabled={selected.length === 0}
            data-qa="new-dm-start"
            className="px-4 py-2 bg-purple-600 hover:bg-purple-500 disabled:bg-purple-600/50 text-white text-sm font-medium rounded-lg transition cursor-pointer disabled:cursor-not-allowed"
          >
            {selected.length > 1 ? 'Start group' : 'Start chat'}
          </button>
        </div>
      </div>
    </Modal>
  );
}
