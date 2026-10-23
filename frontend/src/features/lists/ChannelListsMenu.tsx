import { useRef, useState } from 'react';
import { useNavigate } from 'react-router';
import { Plus, Table2 } from 'lucide-react';
import { AnchoredPopover } from '@/shared/components/Popover/AnchoredPopover';
import { ROUTES } from '@/shared/constants';
import { NewListModal } from './NewListModal';
import { useChannelLists } from './useLists';

interface Props {
  workspaceId: string;
  channelId: string;
  instanceUrl?: string;
  canCreate: boolean;
}

export function ChannelListsMenu({ workspaceId, channelId, instanceUrl, canCreate }: Props) {
  const navigate = useNavigate();
  const anchorRef = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  const [creating, setCreating] = useState(false);
  const lists = useChannelLists(workspaceId, channelId, instanceUrl);
  const count = lists.data?.length ?? 0;

  const openList = (listId: string) => {
    setOpen(false);
    navigate(ROUTES.list(workspaceId, listId));
  };

  return (
    <>
      <button
        ref={anchorRef}
        onClick={() => setOpen((v) => !v)}
        aria-expanded={open}
        aria-haspopup="menu"
        data-qa="channel-lists"
        className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded-md text-xs text-muted hover:text-fg hover:bg-raised/50 transition cursor-pointer"
      >
        <Table2 className="w-3 h-3" />
        Lists
        {count > 0 && <span className="tabular-nums text-subtle">{count}</span>}
      </button>
      {open && (
        <AnchoredPopover
          anchorRef={anchorRef}
          onClose={() => setOpen(false)}
          align="start"
          dataQa="channel-lists-menu"
          className="w-64 bg-surface border border-line rounded-lg shadow-xl py-1"
        >
          <div role="menu" aria-label="Lists">
            {lists.data?.map((list) => (
              <button
                key={list.id}
                role="menuitem"
                onClick={() => openList(list.id)}
                data-qa="channel-list-entry"
                className="w-full px-3 py-2 text-left text-sm text-fg-dim hover:bg-raised flex items-center gap-2 cursor-pointer"
              >
                <Table2 className="w-4 h-4 text-muted shrink-0" />
                <span className="truncate">{list.title}</span>
              </button>
            ))}
            {count === 0 && !lists.isLoading && (
              <p className="px-3 py-2 text-sm text-muted">No lists in this channel yet.</p>
            )}
            {canCreate && (
              <button
                role="menuitem"
                onClick={() => {
                  setOpen(false);
                  setCreating(true);
                }}
                data-autofocus
                data-qa="channel-list-new"
                className="w-full px-3 py-2 text-left text-sm text-fg-dim hover:bg-raised flex items-center gap-2 cursor-pointer border-t border-line/50"
              >
                <Plus className="w-4 h-4 text-muted shrink-0" />
                New list
              </button>
            )}
          </div>
        </AnchoredPopover>
      )}
      {creating && (
        <NewListModal
          workspaceId={workspaceId}
          channelId={channelId}
          instanceUrl={instanceUrl}
          onCreated={(detail) => navigate(ROUTES.list(workspaceId, detail.list.id))}
          onClose={() => setCreating(false)}
        />
      )}
    </>
  );
}
