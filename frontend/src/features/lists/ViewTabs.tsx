import { useRef, useState, type KeyboardEvent } from 'react';
import { ArrowLeft, ArrowRight, Kanban, MoreHorizontal, Pencil, Plus, Table2, Trash2 } from 'lucide-react';
import { IconButton } from '@/shared/components/IconButton/IconButton';
import { TextInput } from '@/shared/components/Form/TextInput';
import { AnchoredPopover } from '@/shared/components/Popover/AnchoredPopover';
import { MenuItem } from './MenuItem';
import { roveMenu } from './roveMenu';
import { MAX_VIEWS, type SavedView, type ViewKind } from './viewModel';

const NAME_MAX = 50;

interface Props {
  views: SavedView[];
  activeId: string;
  canEdit: boolean;
  tabId: (viewId: string) => string;
  panelId: string;
  onSelect: (viewId: string) => void;
  onCreate: (kind: ViewKind) => void;
  onRename: (viewId: string, name: string) => void;
  onMove: (viewId: string, delta: -1 | 1) => void;
  onDelete: (viewId: string) => void;
}

const KIND_ICON = {
  table: <Table2 className="h-3.5 w-3.5" />,
  board: <Kanban className="h-3.5 w-3.5" />,
};

export function ViewTabs({
  views,
  activeId,
  canEdit,
  tabId,
  panelId,
  onSelect,
  onCreate,
  onRename,
  onMove,
  onDelete,
}: Props) {
  const menuRef = useRef<HTMLButtonElement>(null);
  const addRef = useRef<HTMLButtonElement>(null);
  const [menuOpen, setMenuOpen] = useState(false);
  const [adding, setAdding] = useState(false);
  const [renaming, setRenaming] = useState<string | null>(null);
  const index = views.findIndex((view) => view.id === activeId);
  const active = views[index];

  const focusTab = (viewId: string) =>
    requestAnimationFrame(() => document.getElementById(tabId(viewId))?.focus());

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const delta =
      e.key === 'ArrowRight'
        ? 1
        : e.key === 'ArrowLeft'
          ? -1
          : e.key === 'Home'
            ? -index
            : e.key === 'End'
              ? views.length - 1 - index
              : 0;
    if (delta === 0) return;
    e.preventDefault();
    const next = views[(index + delta + views.length) % views.length];
    onSelect(next.id);
    focusTab(next.id);
  };

  const commitName = () => {
    const name = renaming?.trim();
    setRenaming(null);
    if (name && active && name !== active.name) onRename(active.id, name);
    if (active) focusTab(active.id);
  };

  const run = (action: () => void) => () => {
    setMenuOpen(false);
    action();
  };

  return (
    <div className="flex min-w-0 items-center gap-0.5">
      <div
        role="tablist"
        aria-label="Views"
        onKeyDown={onKeyDown}
        className="flex min-w-0 items-center gap-0.5 overflow-x-auto"
      >
        {views.map((view) => {
          const selected = view.id === activeId;
          if (selected && renaming !== null) {
            return (
              <TextInput
                key={view.id}
                label="View name"
                labelHidden
                compact
                autoFocus
                value={renaming}
                maxLength={NAME_MAX}
                onChange={(e) => setRenaming(e.target.value)}
                onBlur={commitName}
                onKeyDown={(e) => {
                  e.stopPropagation();
                  if (e.key === 'Enter') commitName();
                  if (e.key === 'Escape') {
                    setRenaming(null);
                    focusTab(view.id);
                  }
                }}
                dataQa="list-view-name"
                className="w-40"
              />
            );
          }
          return (
            <button
              key={view.id}
              id={tabId(view.id)}
              role="tab"
              aria-selected={selected}
              aria-controls={selected ? panelId : undefined}
              tabIndex={selected ? 0 : -1}
              onClick={() => onSelect(view.id)}
              onDoubleClick={() => canEdit && setRenaming(view.name)}
              data-qa="list-view-tab"
              className={`flex shrink-0 items-center gap-1.5 rounded-md px-2 py-1 text-xs transition cursor-pointer ${
                selected ? 'bg-raised text-fg' : 'text-muted hover:bg-raised/50 hover:text-fg'
              }`}
            >
              {KIND_ICON[view.kind]}
              <span className="max-w-40 truncate">{view.name}</span>
            </button>
          );
        })}
      </div>
      {canEdit && active && (
        <IconButton
          ref={menuRef}
          label={`${active.name} view actions`}
          icon={<MoreHorizontal className="h-4 w-4" />}
          aria-haspopup="menu"
          aria-expanded={menuOpen}
          onClick={() => setMenuOpen((v) => !v)}
          data-qa="list-view-menu"
        />
      )}
      {canEdit && (
        <IconButton
          ref={addRef}
          label="Add view"
          icon={<Plus className="h-4 w-4" />}
          aria-haspopup="menu"
          aria-expanded={adding}
          disabled={views.length >= MAX_VIEWS}
          onClick={() => setAdding((v) => !v)}
          data-qa="list-view-add"
        />
      )}

      {menuOpen && active && (
        <AnchoredPopover
          anchorRef={menuRef}
          onClose={() => setMenuOpen(false)}
          align="start"
          dataQa="list-view-actions"
          className="w-48 bg-surface border border-line rounded-lg shadow-xl p-1"
        >
          <div role="menu" aria-label={`${active.name} view`} onKeyDown={roveMenu}>
            <MenuItem
              first
              icon={<Pencil className="h-4 w-4" />}
              label="Rename"
              onSelect={run(() => setRenaming(active.name))}
              qa="list-view-rename"
            />
            <MenuItem
              icon={<ArrowLeft className="h-4 w-4" />}
              label="Move left"
              disabled={index === 0}
              onSelect={run(() => onMove(active.id, -1))}
              qa="list-view-left"
            />
            <MenuItem
              icon={<ArrowRight className="h-4 w-4" />}
              label="Move right"
              disabled={index === views.length - 1}
              onSelect={run(() => onMove(active.id, 1))}
              qa="list-view-right"
            />
            <MenuItem
              icon={<Trash2 className="h-4 w-4" />}
              label="Delete view"
              danger
              disabled={views.length === 1}
              onSelect={run(() => onDelete(active.id))}
              qa="list-view-delete"
            />
          </div>
        </AnchoredPopover>
      )}

      {adding && (
        <AnchoredPopover
          anchorRef={addRef}
          onClose={() => setAdding(false)}
          align="start"
          dataQa="list-view-add-menu"
          className="w-44 bg-surface border border-line rounded-lg shadow-xl p-1"
        >
          <div role="menu" aria-label="Add view" onKeyDown={roveMenu}>
            <MenuItem
              first
              icon={<Table2 className="h-4 w-4" />}
              label="Table"
              onSelect={() => {
                setAdding(false);
                onCreate('table');
              }}
              qa="list-view-add-table"
            />
            <MenuItem
              icon={<Kanban className="h-4 w-4" />}
              label="Board"
              onSelect={() => {
                setAdding(false);
                onCreate('board');
              }}
              qa="list-view-add-board"
            />
          </div>
        </AnchoredPopover>
      )}
    </div>
  );
}
