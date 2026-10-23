import { useId, useRef, useState } from 'react';
import { useSearchParams } from 'react-router';
import { Eye, Hash, Lock, Menu, MoreHorizontal, Table2, Trash2 } from 'lucide-react';
import type { Channel, WorkspaceMember } from '@/stores/workspace';
import { Spinner } from '@/shared/components/Spinner/Spinner';
import { IconButton } from '@/shared/components/IconButton/IconButton';
import { TextInput } from '@/shared/components/Form/TextInput';
import { AnchoredPopover } from '@/shared/components/Popover/AnchoredPopover';
import { ConfirmDialog } from '@/shared/components/ConfirmDialog/ConfirmDialog';
import { useEscapeToClose } from '@/shared/hooks/useEscapeToClose';
import { useChannelModeration } from '@/features/channel/hooks/useChannelModeration';
import { ListGrid } from './ListGrid';
import { ItemPanel } from './ItemPanel';
import { ColumnDialog } from './ColumnDialog';
import { ListBoard } from './ListBoard';
import { ViewTabs } from './ViewTabs';
import { ViewControls } from './ViewControls';
import { MAX_COLUMNS, MAX_ITEMS, type CellValues, type ListColumn } from './listModel';
import { peopleFrom } from './people';
import {
  applyView,
  nextViewName,
  usableConfig,
  type SavedView,
  type ViewConfig,
  type ViewKind,
} from './viewModel';
import {
  useColumnActions,
  useDeleteList,
  useItemActions,
  useListDetail,
  useListItems,
  useRenameList,
  useViewActions,
} from './useLists';

interface Props {
  workspaceId: string;
  instanceUrl?: string;
  listId: string;
  currentUserId: string;
  channels: Channel[];
  members: WorkspaceMember[];
  onOpenChannel: (channelId: string) => void;
  onOpenNav?: () => void;
}

const TITLE_MAX = 200;

export function ListView({
  workspaceId,
  instanceUrl,
  listId,
  currentUserId,
  channels,
  members,
  onOpenChannel,
  onOpenNav,
}: Props) {
  const detail = useListDetail(listId, instanceUrl);
  const items = useListItems(listId, instanceUrl);
  const [searchParams, setSearchParams] = useSearchParams();
  const list = detail.data?.list;
  const channel = channels.find((c) => c.id === list?.channel_id) ?? null;
  const moderation = useChannelModeration(list?.channel_id ?? null, channel?.settings);
  const canEdit = !!list && (list.channel_id === null || moderation.canPost);
  const canDelete = !!list && (list.created_by === currentUserId || moderation.canModerate);
  const people = peopleFrom(
    members,
    list?.channel_id && moderation.members.length > 0 ? moderation.members.map((m) => m.user_id) : null,
  );

  const columnActions = useColumnActions(listId, instanceUrl);
  const itemActions = useItemActions(listId, instanceUrl);
  const viewActions = useViewActions(listId, instanceUrl);
  const rename = useRenameList(listId, instanceUrl);
  const deleteList = useDeleteList(listId, workspaceId, instanceUrl);

  const [renaming, setRenaming] = useState<string | null>(null);
  const [columnDialog, setColumnDialog] = useState<{ column: ListColumn | null } | null>(null);
  const [deletingColumn, setDeletingColumn] = useState<ListColumn | null>(null);
  const [confirmDeleteList, setConfirmDeleteList] = useState(false);
  const [menuOpen, setMenuOpen] = useState(false);
  const [hiddenOpen, setHiddenOpen] = useState(false);
  const [deletingView, setDeletingView] = useState<SavedView | null>(null);
  const [created, setCreated] = useState<ReadonlySet<string>>(() => new Set());
  const [today] = useState(() => new Date());
  const viewsId = useId();
  const menuRef = useRef<HTMLDivElement>(null);
  const hiddenRef = useRef<HTMLButtonElement>(null);

  const openItemId = searchParams.get('item');
  const setParam = (key: 'item' | 'view', value: string | null) =>
    setSearchParams(
      (params) => {
        const next = new URLSearchParams(params);
        if (value) next.set(key, value);
        else next.delete(key);
        return next;
      },
      { replace: true },
    );
  const setOpenItem = (itemId: string | null) => setParam('item', itemId);
  const selectView = (viewId: string) => setParam('view', viewId);
  const remember = (itemId: string) => setCreated((ids) => new Set(ids).add(itemId));
  const openItem = items.data?.find((item) => item.id === openItemId) ?? null;
  useEscapeToClose(() => setOpenItem(null), !!openItem && !columnDialog && !deletingColumn);

  if (detail.isLoading || (items.isLoading && !detail.isError)) {
    return (
      <main className="flex-1 flex items-center justify-center" aria-label="List">
        <Spinner className="text-accent" />
      </main>
    );
  }

  if (!list || !detail.data || !items.data) {
    return (
      <main
        className="flex-1 flex flex-col items-center justify-center gap-3 px-6 text-center text-muted"
        aria-label="List"
      >
        <Table2 className="h-12 w-12 text-faint" />
        <p className="text-lg font-medium" data-qa="list-missing">
          This list doesn't exist, or you can't see it.
        </p>
      </main>
    );
  }

  const columns = [...detail.data.columns].sort((a, b) => a.position - b.position);
  const visible = columns.filter((column) => !column.hidden);
  const hidden = columns.filter((column) => column.hidden);
  const views = [...detail.data.views].sort((a, b) => a.position - b.position);
  const view = views.find((v) => v.id === searchParams.get('view')) ?? views[0];
  const config = usableConfig(view.config, view.kind, columns);
  const shown = applyView(
    items.data,
    columns,
    config,
    { me: currentUserId, today, nameOf: people.nameOf },
    created,
  );
  const group = columns.find((column) => column.id === config.group_by) ?? null;
  const total = items.data.length;
  const canAddItem = total < MAX_ITEMS;
  const tabId = (viewId: string) => `${viewsId}-tab-${viewId}`;
  const panelId = `${viewsId}-panel`;

  const addItem = async (values: CellValues, afterId: string | null) => {
    const item = await itemActions.create.mutateAsync({ values, after_id: afterId }).catch(() => undefined);
    if (item) remember(item.id);
    return item;
  };

  const createView = (kind: ViewKind) => {
    const firstSelect = columns.find((column) => column.kind === 'select');
    viewActions.create.mutate(
      {
        name: nextViewName(views, kind),
        kind,
        config: kind === 'board' && firstSelect ? { group_by: firstSelect.id } : {},
      },
      { onSuccess: (created) => selectView(created.id) },
    );
  };

  const moveView = (viewId: string, delta: -1 | 1) => {
    const order = views.map((v) => v.id);
    const from = order.indexOf(viewId);
    const to = from + delta;
    if (to < 0 || to >= order.length) return;
    order.splice(from, 1);
    order.splice(to, 0, viewId);
    viewActions.reorder.mutate(order);
  };

  const changeConfig = (next: ViewConfig) =>
    viewActions.update.mutate({ viewId: view.id, body: { config: next } });

  const commitTitle = () => {
    const title = renaming?.trim();
    setRenaming(null);
    if (title && title !== list.title) rename.mutate(title);
  };

  const moveColumn = (column: ListColumn, delta: -1 | 1) => {
    const at = visible.findIndex((c) => c.id === column.id);
    const neighbour = visible[at + delta];
    if (!neighbour) return;
    const order = columns.map((c) => c.id);
    const from = order.indexOf(column.id);
    const to = order.indexOf(neighbour.id);
    order.splice(from, 1);
    order.splice(to, 0, column.id);
    columnActions.reorder.mutate(order);
  };

  return (
    <div className="flex-1 flex min-w-0">
      <main className="flex-1 flex flex-col min-w-0" aria-label={`List ${list.title}`} data-qa="list-view">
        <div className="h-14 px-4 flex items-center gap-2 border-b border-line/50 bg-surface/30 shrink-0">
          {onOpenNav && (
            <IconButton
              label="Open navigation"
              icon={<Menu className="w-5 h-5" />}
              size="md"
              className="lg:hidden -ml-1"
              onClick={onOpenNav}
            />
          )}
          <Table2 className="w-4 h-4 text-muted shrink-0" />
          {renaming !== null ? (
            <TextInput
              label="List title"
              labelHidden
              compact
              autoFocus
              value={renaming}
              maxLength={TITLE_MAX}
              onChange={(e) => setRenaming(e.target.value)}
              onBlur={commitTitle}
              onKeyDown={(e) => {
                if (e.key === 'Enter') commitTitle();
                if (e.key === 'Escape') {
                  e.stopPropagation();
                  setRenaming(null);
                }
              }}
              dataQa="list-title-input"
              className="w-72"
            />
          ) : (
            <h1 className="min-w-0">
              <button
                onClick={() => canEdit && setRenaming(list.title)}
                disabled={!canEdit}
                data-qa="list-title"
                title={canEdit ? 'Rename the list' : undefined}
                className="max-w-full truncate rounded px-1 font-semibold text-fg enabled:cursor-text enabled:hover:bg-raised/50"
              >
                {list.title}
              </button>
            </h1>
          )}
          {channel && (
            <button
              onClick={() => onOpenChannel(channel.id)}
              aria-label={`Open ${channel.name}`}
              data-qa="list-channel"
              className="max-sm:hidden shrink-0 flex items-center gap-0.5 px-1.5 py-0.5 rounded text-xs text-muted hover:text-fg hover:bg-raised/50 cursor-pointer"
            >
              in
              {channel.channel_type === 'private' ? (
                <Lock className="w-3 h-3" />
              ) : (
                <Hash className="w-3 h-3" />
              )}
              {channel.name}
            </button>
          )}
          <span className="text-xs text-subtle tabular-nums shrink-0" data-qa="list-count">
            {shown.length === total ? '' : `${shown.length} of `}
            {total} {total === 1 ? 'item' : 'items'}
          </span>

          <div className="ml-auto flex items-center gap-1">
            {hidden.length > 0 && (
              <button
                ref={hiddenRef}
                onClick={() => setHiddenOpen((v) => !v)}
                aria-expanded={hiddenOpen}
                data-qa="list-hidden-columns"
                className="flex items-center gap-1 px-2 py-1 rounded-md text-xs text-muted hover:text-fg hover:bg-raised/50 cursor-pointer"
              >
                <Eye className="w-3.5 h-3.5" />
                {hidden.length} hidden
              </button>
            )}
            {canDelete && (
              <div ref={menuRef}>
                <IconButton
                  label="List actions"
                  icon={<MoreHorizontal className="w-4 h-4" />}
                  size="md"
                  onClick={() => setMenuOpen((v) => !v)}
                  data-qa="list-actions"
                />
              </div>
            )}
          </div>
        </div>

        <div className="min-h-11 px-3 py-1 flex flex-wrap items-center gap-x-2 gap-y-1 border-b border-line/50 shrink-0">
          <ViewTabs
            views={views}
            activeId={view.id}
            canEdit={canEdit}
            tabId={tabId}
            panelId={panelId}
            onSelect={selectView}
            onCreate={createView}
            onRename={(viewId, name) => viewActions.update.mutate({ viewId, body: { name } })}
            onMove={moveView}
            onDelete={(viewId) => setDeletingView(views.find((v) => v.id === viewId) ?? null)}
          />
          <div className="ml-auto">
            <ViewControls
              key={view.id}
              kind={view.kind}
              config={config}
              columns={columns}
              people={people}
              canEdit={canEdit}
              onChange={changeConfig}
            />
          </div>
        </div>

        <div
          id={panelId}
          role="tabpanel"
          aria-labelledby={tabId(view.id)}
          className="flex min-h-0 flex-1 flex-col"
        >
          {view.kind === 'board' ? (
            <ListBoard
              label={list.title}
              group={group}
              columns={visible}
              items={shown}
              canEdit={canEdit}
              canAdd={canAddItem}
              hasGroupable={columns.some((column) => column.kind === 'select')}
              people={people}
              openItemId={openItemId}
              onOpenItem={setOpenItem}
              onSetGroup={(itemId, choiceId) =>
                group && itemActions.update.mutate({ itemId, values: { [group.id]: choiceId } })
              }
              onAddItem={async (choiceId) => {
                const values: CellValues = group && choiceId ? { [group.id]: choiceId } : {};
                const item = await addItem(values, items.data?.at(-1)?.id ?? null);
                if (item) setOpenItem(item.id);
              }}
            />
          ) : (
            <ListGrid
              key={view.id}
              label={list.title}
              columns={visible}
              items={shown}
              canEdit={canEdit}
              reorderable={config.sort.length === 0 && config.filters.length === 0}
              canAdd={canAddItem}
              emptyText={total === 0 ? 'No items yet.' : 'No items match this view.'}
              people={people}
              openItemId={openItemId}
              columnActions={{
                onEdit: (column) => setColumnDialog({ column }),
                onMove: moveColumn,
                onHide: (column) => columnActions.setHidden.mutate({ columnId: column.id, hidden: true }),
                onDelete: setDeletingColumn,
                onAdd: () => setColumnDialog({ column: null }),
                canAdd: columns.length < MAX_COLUMNS,
              }}
              onEditCell={(itemId, columnId, value) =>
                itemActions.update.mutate({ itemId, values: { [columnId]: value } })
              }
              onAddItem={(afterId) => addItem({}, afterId)}
              onOpenItem={setOpenItem}
              onMoveItem={(itemId, afterId) => itemActions.move.mutate({ itemId, afterId })}
            />
          )}
        </div>
      </main>

      {openItem && (
        <ItemPanel
          key={openItem.id}
          item={openItem}
          columns={columns}
          canEdit={canEdit}
          people={people}
          onEdit={(columnId, value) =>
            itemActions.update.mutate({ itemId: openItem.id, values: { [columnId]: value } })
          }
          onDelete={() => {
            itemActions.remove.mutate(openItem.id);
            setOpenItem(null);
          }}
          onClose={() => setOpenItem(null)}
        />
      )}

      {hiddenOpen && hidden.length > 0 && (
        <AnchoredPopover
          anchorRef={hiddenRef}
          onClose={() => setHiddenOpen(false)}
          dataQa="list-hidden-menu"
          className="w-56 bg-surface border border-line rounded-lg shadow-xl p-1"
        >
          {hidden.map((column, index) => (
            <button
              key={column.id}
              data-autofocus={index === 0 || undefined}
              disabled={!canEdit}
              onClick={() => columnActions.setHidden.mutate({ columnId: column.id, hidden: false })}
              data-qa="list-show-column"
              className="w-full px-3 py-2 text-left text-sm text-fg-dim rounded hover:bg-raised flex items-center gap-2 cursor-pointer disabled:cursor-default"
            >
              <Eye className="w-4 h-4 text-muted" />
              <span className="truncate">Show {column.name}</span>
            </button>
          ))}
        </AnchoredPopover>
      )}

      {menuOpen && (
        <AnchoredPopover
          anchorRef={menuRef}
          onClose={() => setMenuOpen(false)}
          className="w-48 bg-surface border border-line rounded-lg shadow-xl p-1"
        >
          <button
            data-autofocus
            onClick={() => {
              setMenuOpen(false);
              setConfirmDeleteList(true);
            }}
            data-qa="list-delete"
            className="w-full px-3 py-2 text-left text-sm text-danger rounded hover:bg-raised flex items-center gap-2 cursor-pointer"
          >
            <Trash2 className="w-4 h-4" />
            Delete list
          </button>
        </AnchoredPopover>
      )}

      {columnDialog && (
        <ColumnDialog
          column={columnDialog.column}
          actions={columnActions}
          onClose={() => setColumnDialog(null)}
        />
      )}

      {deletingColumn && (
        <ConfirmDialog
          title={`Delete ${deletingColumn.name}?`}
          description="The column and every value in it will be removed for everyone."
          confirmLabel="Delete column"
          onConfirm={() => columnActions.remove.mutateAsync(deletingColumn.id).then(() => undefined)}
          onClose={() => setDeletingColumn(null)}
          dataQa="list-column-delete-confirm"
        />
      )}

      {deletingView && (
        <ConfirmDialog
          title={`Delete the ${deletingView.name} view?`}
          description="The view is removed for everyone. Items stay in the list."
          confirmLabel="Delete view"
          onConfirm={async () => {
            if (deletingView.id === view.id) {
              selectView(views.find((v) => v.id !== deletingView.id)?.id ?? '');
            }
            await viewActions.remove.mutateAsync(deletingView.id);
          }}
          onClose={() => setDeletingView(null)}
          dataQa="list-view-delete-confirm"
        />
      )}

      {confirmDeleteList && (
        <ConfirmDialog
          title={`Delete ${list.title}?`}
          description="The list and all of its items will be removed for everyone."
          confirmLabel="Delete list"
          onConfirm={async () => {
            await deleteList.mutateAsync();
            if (channel) onOpenChannel(channel.id);
          }}
          onClose={() => setConfirmDeleteList(false)}
          dataQa="list-delete-confirm"
        />
      )}
    </div>
  );
}
