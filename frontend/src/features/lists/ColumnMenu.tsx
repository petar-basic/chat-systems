import { useRef, type ReactNode } from 'react';
import {
  ArrowLeft,
  ArrowRight,
  Calendar,
  CheckSquare,
  ChevronDown,
  CircleDot,
  EyeOff,
  Hash,
  Link,
  ListChecks,
  Pencil,
  Plus,
  Trash2,
  Type,
  User,
} from 'lucide-react';
import { AnchoredPopover } from '@/shared/components/Popover/AnchoredPopover';
import type { ColumnKind, ListColumn } from './listModel';
import { MenuItem } from './MenuItem';
import { roveMenu } from './roveMenu';

export interface ColumnActions {
  onEdit: (column: ListColumn) => void;
  onMove: (column: ListColumn, delta: -1 | 1) => void;
  onHide: (column: ListColumn) => void;
  onDelete: (column: ListColumn) => void;
  onAdd: () => void;
  canAdd: boolean;
}

const KIND_ICONS: Record<ColumnKind, ReactNode> = {
  text: <Type className="h-3.5 w-3.5" />,
  number: <Hash className="h-3.5 w-3.5" />,
  select: <CircleDot className="h-3.5 w-3.5" />,
  multi_select: <ListChecks className="h-3.5 w-3.5" />,
  person: <User className="h-3.5 w-3.5" />,
  date: <Calendar className="h-3.5 w-3.5" />,
  checkbox: <CheckSquare className="h-3.5 w-3.5" />,
  link: <Link className="h-3.5 w-3.5" />,
};

interface Props {
  column: ListColumn;
  index: number;
  count: number;
  canEdit: boolean;
  open: boolean;
  onOpen: () => void;
  onClose: () => void;
  actions: ColumnActions;
}

export function ColumnMenu({ column, index, count, canEdit, open, onOpen, onClose, actions }: Props) {
  const anchorRef = useRef<HTMLButtonElement>(null);
  const run = (action: () => void) => () => {
    onClose();
    action();
  };

  return (
    <>
      <button
        ref={anchorRef}
        tabIndex={-1}
        onClick={canEdit ? onOpen : undefined}
        aria-haspopup={canEdit ? 'menu' : undefined}
        aria-expanded={canEdit ? open : undefined}
        data-qa="list-column-header"
        className={`flex h-full w-full min-w-0 items-center gap-1.5 px-2 text-left text-xs font-medium text-muted ${
          canEdit ? 'cursor-pointer hover:bg-raised/50 hover:text-fg' : 'cursor-default'
        }`}
      >
        <span className="shrink-0">{KIND_ICONS[column.kind]}</span>
        <span className="truncate">{column.name}</span>
        {canEdit && <ChevronDown className="ml-auto h-3 w-3 shrink-0" />}
      </button>
      {open && (
        <AnchoredPopover
          anchorRef={anchorRef}
          onClose={onClose}
          align="start"
          dataQa="list-column-menu"
          className="w-52 bg-surface border border-line rounded-lg shadow-xl p-1"
        >
          <div
            role="menu"
            aria-label={`${column.name} column`}
            onKeyDown={(e) => {
              if (e.key === 'Escape') {
                e.preventDefault();
                e.stopPropagation();
                onClose();
                return;
              }
              roveMenu(e);
            }}
          >
            <MenuItem
              first
              icon={<Pencil className="h-4 w-4" />}
              label="Edit column…"
              onSelect={run(() => actions.onEdit(column))}
              qa="list-column-edit"
            />
            <MenuItem
              icon={<ArrowLeft className="h-4 w-4" />}
              label="Move left"
              disabled={index === 0}
              onSelect={run(() => actions.onMove(column, -1))}
              qa="list-column-left"
            />
            <MenuItem
              icon={<ArrowRight className="h-4 w-4" />}
              label="Move right"
              disabled={index === count - 1}
              onSelect={run(() => actions.onMove(column, 1))}
              qa="list-column-right"
            />
            <MenuItem
              icon={<EyeOff className="h-4 w-4" />}
              label="Hide column"
              onSelect={run(() => actions.onHide(column))}
              qa="list-column-hide"
            />
            <MenuItem
              icon={<Plus className="h-4 w-4" />}
              label="Add column"
              disabled={!actions.canAdd}
              onSelect={run(actions.onAdd)}
              qa="list-column-add-from-menu"
            />
            <MenuItem
              icon={<Trash2 className="h-4 w-4" />}
              label="Delete column"
              danger
              onSelect={run(() => actions.onDelete(column))}
              qa="list-column-delete"
            />
          </div>
        </AnchoredPopover>
      )}
    </>
  );
}
