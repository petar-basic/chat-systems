import { useEffect, useId, useRef, useState, type DragEvent, type KeyboardEvent } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { GripVertical, Maximize2, MessageSquare, Plus } from 'lucide-react';
import { IconButton } from '@/shared/components/IconButton/IconButton';
import { CellContent } from './CellContent';
import { CellEditor, type Move } from './CellEditor';
import { ColumnMenu, type ColumnActions } from './ColumnMenu';
import { COLUMN_WIDTHS, typesText, type ListColumn, type ListItem } from './listModel';
import type { People } from './people';

const ROW_HEIGHT = 36;
const HEADER_HEIGHT = 36;
const LEAD_WIDTH = 88;
const PAGE = 10;

interface Props {
  label: string;
  columns: ListColumn[];
  items: ListItem[];
  canEdit: boolean;
  reorderable: boolean;
  canAdd: boolean;
  emptyText: string;
  people: People;
  columnActions: ColumnActions;
  openItemId: string | null;
  onEditCell: (itemId: string, columnId: string, value: unknown) => void;
  onAddItem: (afterId: string | null) => Promise<ListItem | undefined>;
  onOpenItem: (itemId: string) => void;
  onMoveItem: (itemId: string, afterId: string | null) => void;
}

interface Spot {
  itemId: string | null;
  columnId: string;
}

interface Editing {
  itemId: string;
  columnId: string;
  draft: string | null;
}

const sameValue = (a: unknown, b: unknown) => JSON.stringify(a ?? null) === JSON.stringify(b ?? null);
const clamp = (n: number, low: number, high: number) => Math.min(high, Math.max(low, n));
const opensInPopover = (column: ListColumn) => column.kind === 'person' || column.kind === 'multi_select';

export function ListGrid({
  label,
  columns,
  items,
  canEdit,
  reorderable,
  canAdd,
  emptyText,
  people,
  columnActions,
  openItemId,
  onEditCell,
  onAddItem,
  onOpenItem,
  onMoveItem,
}: Props) {
  const gridId = useId();
  const gridRef = useRef<HTMLDivElement>(null);
  const editorAnchorRef = useRef<HTMLDivElement>(null);
  const revealRef = useRef<string | null>(null);
  const [active, setActive] = useState<Spot | null>(null);
  const [editing, setEditing] = useState<Editing | null>(null);
  const [menuColumnId, setMenuColumnId] = useState<string | null>(null);
  const [dragging, setDragging] = useState<string | null>(null);
  const [drop, setDrop] = useState<{ itemId: string; after: boolean } | null>(null);

  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => gridRef.current,
    estimateSize: () => ROW_HEIGHT,
    getItemKey: (index) => items[index]?.id ?? index,
    scrollMargin: HEADER_HEIGHT,
    scrollPaddingStart: HEADER_HEIGHT,
    overscan: 12,
  });

  useEffect(() => {
    const target = revealRef.current;
    if (!target) return;
    const index = items.findIndex((item) => item.id === target);
    if (index < 0) return;
    revealRef.current = null;
    virtualizer.scrollToIndex(index, { align: 'auto' });
  }, [items, virtualizer]);

  const widths = columns.map((column) => COLUMN_WIDTHS[column.kind]);
  const totalWidth = LEAD_WIDTH + widths.reduce((sum, w) => sum + w, 0) + (canEdit ? 44 : 0);

  const colIndex = Math.max(
    0,
    columns.findIndex((column) => column.id === active?.columnId),
  );
  const rowIndex =
    active === null
      ? items.length > 0
        ? 0
        : -1
      : active.itemId === null
        ? -1
        : items.findIndex((item) => item.id === active.itemId);

  const cellId = (itemId: string, columnId: string) => `${gridId}-${itemId}-${columnId}`;
  const headerId = (columnId: string) => `${gridId}-head-${columnId}`;
  const activeId =
    columns.length === 0
      ? undefined
      : rowIndex === -1
        ? headerId(columns[colIndex].id)
        : cellId(items[rowIndex].id, columns[colIndex].id);

  const refocus = () => gridRef.current?.focus({ preventScroll: true });

  const go = (row: number, col: number) => {
    if (columns.length === 0) return;
    const r = clamp(row, -1, items.length - 1);
    const c = clamp(col, 0, columns.length - 1);
    const itemId = r === -1 ? null : items[r].id;
    const columnId = columns[c].id;
    setActive({ itemId, columnId });
    if (r >= 0) virtualizer.scrollToIndex(r, { align: 'auto' });
    requestAnimationFrame(() =>
      document
        .getElementById(itemId === null ? headerId(columnId) : cellId(itemId, columnId))
        ?.scrollIntoView({ block: 'nearest', inline: 'nearest' }),
    );
  };

  const step = (row: number, col: number, move: Move) => {
    if (move === 'down') return go(Math.min(row + 1, items.length - 1), col);
    if (move === 'right') {
      if (col < columns.length - 1) return go(row, col + 1);
      if (row < items.length - 1) return go(row + 1, 0);
    }
    if (move === 'left') {
      if (col > 0) return go(row, col - 1);
      if (row > 0) return go(row - 1, columns.length - 1);
    }
  };

  const beginEdit = (row: number, col: number, draft: string | null) => {
    if (!canEdit || row < 0) return;
    const item = items[row];
    const column = columns[col];
    if (!item || !column) return;
    if (column.kind === 'checkbox') {
      onEditCell(item.id, column.id, item.values[column.id] !== true);
      return;
    }
    setActive({ itemId: item.id, columnId: column.id });
    setEditing({ itemId: item.id, columnId: column.id, draft });
  };

  const commit = (value: unknown, move: Move) => {
    if (!editing) return;
    const row = items.findIndex((item) => item.id === editing.itemId);
    const col = columns.findIndex((column) => column.id === editing.columnId);
    setEditing(null);
    refocus();
    const item = items[row];
    if (item && !sameValue(item.values[editing.columnId], value))
      onEditCell(item.id, editing.columnId, value);
    if (move && row >= 0 && col >= 0) step(row, col, move);
  };

  const cancel = () => {
    setEditing(null);
    refocus();
  };

  const moveRow = (row: number, delta: -1 | 1) => {
    const item = items[row];
    if (!canEdit || !reorderable || !item) return;
    if (delta === -1 && row > 0) onMoveItem(item.id, items[row - 2]?.id ?? null);
    if (delta === 1 && row < items.length - 1) onMoveItem(item.id, items[row + 1].id);
    requestAnimationFrame(() =>
      document
        .getElementById(cellId(item.id, columns[colIndex].id))
        ?.scrollIntoView({ block: 'nearest', inline: 'nearest' }),
    );
  };

  const addItem = async () => {
    const created = await onAddItem(items[items.length - 1]?.id ?? null);
    if (!created || columns.length === 0) return;
    revealRef.current = created.id;
    setActive({ itemId: created.id, columnId: columns[0].id });
    if (columns[0].kind !== 'checkbox')
      setEditing({ itemId: created.id, columnId: columns[0].id, draft: null });
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.target !== e.currentTarget || editing || columns.length === 0) return;
    const row = rowIndex;
    const col = colIndex;
    const column = columns[col];
    const item = row >= 0 ? items[row] : undefined;
    const handled = () => e.preventDefault();

    if (e.altKey && (e.key === 'ArrowUp' || e.key === 'ArrowDown')) {
      handled();
      moveRow(row, e.key === 'ArrowUp' ? -1 : 1);
      return;
    }
    switch (e.key) {
      case 'ArrowUp':
        handled();
        return go(row - 1, col);
      case 'ArrowDown':
        handled();
        return go(row + 1, col);
      case 'ArrowLeft':
        handled();
        return go(row, col - 1);
      case 'ArrowRight':
        handled();
        return go(row, col + 1);
      case 'Home':
        handled();
        return go(e.ctrlKey || e.metaKey ? 0 : row, 0);
      case 'End':
        handled();
        return go(e.ctrlKey || e.metaKey ? items.length - 1 : row, columns.length - 1);
      case 'PageDown':
        handled();
        return go(row + PAGE, col);
      case 'PageUp':
        handled();
        return go(Math.max(0, row - PAGE), col);
      case 'Tab': {
        const next = e.shiftKey ? col - 1 : col + 1;
        if (next >= 0 && next < columns.length) {
          handled();
          return go(row, next);
        }
        if (!e.shiftKey && row < items.length - 1) {
          handled();
          return go(row + 1, 0);
        }
        if (e.shiftKey && row > 0) {
          handled();
          return go(row - 1, columns.length - 1);
        }
        return;
      }
      case 'Enter':
      case 'F2':
        handled();
        if (row === -1) {
          if (canEdit) setMenuColumnId(column.id);
          return;
        }
        if (e.shiftKey && item) return onOpenItem(item.id);
        return beginEdit(row, col, null);
      case 'Delete':
      case 'Backspace':
        if (!canEdit || !item) return;
        handled();
        if (item.values[column.id] !== undefined) onEditCell(item.id, column.id, null);
        return;
    }
    if (e.key === ' ' && row === -1 && canEdit) {
      handled();
      setMenuColumnId(column.id);
      return;
    }
    if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey && row >= 0) {
      handled();
      const draft = typesText(column.kind) || column.kind === 'person' ? e.key : null;
      beginEdit(row, col, e.key === ' ' && column.kind !== 'text' ? null : draft);
    }
  };

  const onRowDragOver = (itemId: string) => (e: DragEvent<HTMLDivElement>) => {
    if (!dragging) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = 'move';
    const rect = e.currentTarget.getBoundingClientRect();
    const after = e.clientY > rect.top + rect.height / 2;
    if (drop?.itemId !== itemId || drop.after !== after) setDrop({ itemId, after });
  };

  const finishDrag = () => {
    setDragging(null);
    setDrop(null);
  };

  const onRowDrop = (e: DragEvent<HTMLDivElement>) => {
    e.preventDefault();
    if (dragging && drop && dragging !== drop.itemId) {
      const at = items.findIndex((item) => item.id === drop.itemId);
      const afterId = drop.after ? drop.itemId : (items[at - 1]?.id ?? null);
      const from = items.findIndex((item) => item.id === dragging);
      const currentAfter = items[from - 1]?.id ?? null;
      if (afterId !== dragging && afterId !== currentAfter) onMoveItem(dragging, afterId);
    }
    finishDrag();
  };

  const editingColumn = editing ? columns.find((column) => column.id === editing.columnId) : undefined;

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div
        ref={gridRef}
        role="grid"
        aria-label={label}
        aria-rowcount={items.length + 1}
        aria-colcount={columns.length + 1}
        aria-activedescendant={activeId}
        aria-readonly={!canEdit || undefined}
        tabIndex={0}
        onKeyDown={onKeyDown}
        data-qa="list-grid"
        style={{ scrollPaddingTop: HEADER_HEIGHT, scrollPaddingLeft: LEAD_WIDTH }}
        className="group/grid relative min-h-0 flex-1 overflow-auto focus:outline-none"
      >
        <div role="rowgroup" className="sticky top-0 z-20" style={{ width: totalWidth }}>
          <div
            role="row"
            aria-rowindex={1}
            className="flex border-b border-line bg-surface"
            style={{ height: HEADER_HEIGHT }}
          >
            <div
              role="columnheader"
              aria-colindex={1}
              className="sticky left-0 z-10 shrink-0 border-r border-line/40 bg-surface"
              style={{ width: LEAD_WIDTH }}
            >
              <span className="sr-only">Row</span>
            </div>
            {columns.map((column, c) => {
              const isActive = rowIndex === -1 && colIndex === c;
              return (
                <div
                  key={column.id}
                  id={headerId(column.id)}
                  role="columnheader"
                  aria-colindex={c + 2}
                  aria-selected={isActive}
                  className={`shrink-0 border-r border-line/40 ${isActive ? 'ring-2 ring-inset ring-purple-500/70' : ''}`}
                  style={{ width: widths[c] }}
                  onClick={() => setActive({ itemId: null, columnId: column.id })}
                >
                  <ColumnMenu
                    column={column}
                    index={c}
                    count={columns.length}
                    canEdit={canEdit}
                    open={menuColumnId === column.id}
                    onOpen={() => setMenuColumnId(column.id)}
                    onClose={() => {
                      setMenuColumnId(null);
                      refocus();
                    }}
                    actions={columnActions}
                  />
                </div>
              );
            })}
            {canEdit && (
              <div className="flex w-11 shrink-0 items-center justify-center">
                <IconButton
                  label="Add column"
                  icon={<Plus className="h-4 w-4" />}
                  tabIndex={-1}
                  disabled={!columnActions.canAdd}
                  onClick={columnActions.onAdd}
                  data-qa="list-column-add"
                />
              </div>
            )}
          </div>
        </div>

        <div
          role="rowgroup"
          className="relative"
          style={{ height: virtualizer.getTotalSize(), width: totalWidth }}
        >
          {virtualizer.getVirtualItems().map((virtual) => {
            const item = items[virtual.index];
            if (!item) return null;
            const dropEdge =
              drop?.itemId === item.id && dragging !== item.id ? (drop.after ? 'after' : 'before') : null;
            return (
              <div
                key={item.id}
                role="row"
                aria-rowindex={virtual.index + 2}
                data-qa="list-row"
                data-item-id={item.id}
                onDragOver={onRowDragOver(item.id)}
                onDrop={onRowDrop}
                className={`group/row absolute left-0 top-0 flex ${dragging === item.id ? 'opacity-50' : ''} ${
                  openItemId === item.id ? 'bg-purple-500/5' : ''
                } ${dropEdge === 'after' ? 'shadow-[inset_0_-2px_0_var(--color-purple-500)]' : ''} ${
                  dropEdge === 'before' ? 'shadow-[inset_0_2px_0_var(--color-purple-500)]' : ''
                }`}
                style={{
                  height: ROW_HEIGHT,
                  width: totalWidth,
                  transform: `translateY(${virtual.start - HEADER_HEIGHT}px)`,
                }}
              >
                <div
                  role="rowheader"
                  aria-colindex={1}
                  className="sticky left-0 z-10 flex shrink-0 items-center gap-0.5 border-b border-r border-line/40 bg-surface pl-1 text-xs text-subtle"
                  style={{ width: LEAD_WIDTH }}
                >
                  {canEdit && reorderable ? (
                    <span
                      draggable
                      onDragStart={(e) => {
                        e.dataTransfer.effectAllowed = 'move';
                        e.dataTransfer.setData('text/plain', item.id);
                        setDragging(item.id);
                      }}
                      onDragEnd={finishDrag}
                      aria-hidden
                      data-qa="list-row-grip"
                      className="cursor-grab p-0.5 text-subtle opacity-0 group-hover/row:opacity-100 active:cursor-grabbing"
                    >
                      <GripVertical className="h-3.5 w-3.5" />
                    </span>
                  ) : (
                    <span className="w-4.5" />
                  )}
                  <span className="w-9 text-right tabular-nums">{virtual.index + 1}</span>
                  {item.comment_count > 0 ? (
                    <button
                      type="button"
                      tabIndex={-1}
                      aria-label={`Open row ${virtual.index + 1}, ${item.comment_count} ${
                        item.comment_count === 1 ? 'comment' : 'comments'
                      }`}
                      onClick={() => onOpenItem(item.id)}
                      data-qa="list-row-comments"
                      className="flex h-7 w-7 items-center justify-center gap-0.5 rounded text-[11px] tabular-nums text-muted hover:bg-raised/60 hover:text-fg cursor-pointer"
                    >
                      <MessageSquare className="h-3 w-3" />
                      {item.comment_count > 99 ? '99+' : item.comment_count}
                    </button>
                  ) : (
                    <IconButton
                      label={`Open row ${virtual.index + 1}`}
                      icon={<Maximize2 className="h-3.5 w-3.5" />}
                      tabIndex={-1}
                      className="opacity-0 group-hover/row:opacity-100 focus:opacity-100"
                      onClick={() => onOpenItem(item.id)}
                      data-qa="list-row-open"
                    />
                  )}
                </div>
                {columns.map((column, c) => {
                  const isActive = rowIndex === virtual.index && colIndex === c;
                  const isEditing = editing?.itemId === item.id && editing.columnId === column.id;
                  const inline = isEditing && !opensInPopover(column);
                  const value = item.values[column.id];
                  return (
                    <div
                      key={column.id}
                      id={cellId(item.id, column.id)}
                      ref={isEditing ? editorAnchorRef : undefined}
                      role="gridcell"
                      aria-colindex={c + 2}
                      aria-selected={isActive}
                      data-qa="list-cell"
                      data-column={column.name}
                      onClick={() => {
                        if (isEditing) return;
                        if (column.kind === 'checkbox' && canEdit) {
                          setActive({ itemId: item.id, columnId: column.id });
                          onEditCell(item.id, column.id, value !== true);
                          return;
                        }
                        if (isActive) beginEdit(virtual.index, c, null);
                        else setActive({ itemId: item.id, columnId: column.id });
                      }}
                      onDoubleClick={() => {
                        if (!isEditing && column.kind !== 'checkbox') beginEdit(virtual.index, c, null);
                      }}
                      className={`relative flex shrink-0 items-center border-b border-r border-line/40 text-sm text-fg-soft ${
                        inline ? 'z-10 overflow-visible p-0.5' : 'overflow-hidden px-2'
                      } ${isActive ? 'ring-2 ring-inset ring-purple-500/70 group-focus/grid:ring-purple-500' : ''}`}
                      style={{ width: widths[c] }}
                    >
                      {!inline && <CellContent column={column} value={value} people={people} />}
                      {isEditing && editingColumn && (
                        <CellEditor
                          column={editingColumn}
                          value={value}
                          initialDraft={editing.draft}
                          people={people}
                          anchorRef={editorAnchorRef}
                          onCommit={commit}
                          onCancel={cancel}
                        />
                      )}
                    </div>
                  );
                })}
              </div>
            );
          })}
        </div>

        {items.length === 0 && (
          <p className="sticky left-0 px-4 py-6 text-sm text-muted" data-qa="list-empty">
            {columns.length === 0 ? 'Every column is hidden.' : emptyText}
          </p>
        )}
      </div>

      {canEdit && (
        <div className="shrink-0 border-t border-line/50 px-3 py-1.5">
          <button
            onClick={() => void addItem()}
            disabled={!canAdd}
            data-qa="list-add-item"
            className="inline-flex items-center gap-1.5 rounded-md px-2 py-1 text-sm text-muted transition hover:bg-raised/50 hover:text-fg disabled:cursor-not-allowed disabled:opacity-50 cursor-pointer"
          >
            <Plus className="h-4 w-4" />
            New item
          </button>
        </div>
      )}
    </div>
  );
}
