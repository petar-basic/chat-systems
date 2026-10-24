import { describe, it, expect, vi, beforeAll, beforeEach } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { ListGrid } from './ListGrid';
import { peopleFrom } from './people';
import type { ColumnActions } from './ColumnMenu';
import type { ListColumn, ListItem } from './listModel';
import type { WorkspaceMember } from '@/stores/workspace';

const column = (id: string, name: string, kind: ListColumn['kind'], options: ListColumn['options'] = {}) =>
  ({ id, list_id: 'l-1', position: 0, name, kind, options, hidden: false }) satisfies ListColumn;

const COLUMNS: ListColumn[] = [
  column('c-text', 'Title', 'text'),
  column('c-num', 'Points', 'number'),
  column('c-status', 'Status', 'select', {
    choices: [
      { id: 's-todo', label: 'To do', color: 'gray' },
      { id: 's-done', label: 'Done', color: 'green' },
    ],
  }),
  column('c-tags', 'Tags', 'multi_select', {
    choices: [
      { id: 't-a', label: 'Alpha', color: 'blue' },
      { id: 't-b', label: 'Beta', color: 'red' },
    ],
  }),
  column('c-owner', 'Owner', 'person'),
  column('c-due', 'Due', 'date'),
  column('c-ok', 'Finished', 'checkbox'),
  column('c-link', 'Link', 'link'),
];

const item = (id: string, rank: string, values: ListItem['values'] = {}): ListItem => ({
  id,
  list_id: 'l-1',
  rank,
  values,
  created_by: 'u-ana',
  created_at: '2026-10-01T00:00:00Z',
  updated_at: '2026-10-01T00:00:00Z',
});

const ITEMS = [
  item('i-1', 'a', { 'c-text': 'First', 'c-status': 's-todo' }),
  item('i-2', 'b', { 'c-text': 'Second' }),
  item('i-3', 'c'),
];

const member = (user_id: string, display_name: string) =>
  ({ user_id, display_name, email: `${display_name}@x.dev`, role: 'member' }) as WorkspaceMember;
const PEOPLE = peopleFrom([member('u-ana', 'Ana'), member('u-bob', 'Bob')], null);

const onEditCell = vi.fn<(itemId: string, columnId: string, value: unknown) => void>();
const onAddItem = vi.fn<(afterId: string | null) => Promise<ListItem | undefined>>();
const onOpenItem = vi.fn<(itemId: string) => void>();
const onMoveItem = vi.fn<(itemId: string, afterId: string | null) => void>();
const columnActions = {
  onEdit: vi.fn(),
  onMove: vi.fn(),
  onHide: vi.fn(),
  onDelete: vi.fn(),
  onAdd: vi.fn(),
  canAdd: true,
} satisfies ColumnActions;

beforeAll(() => {
  Object.defineProperty(HTMLElement.prototype, 'clientHeight', { configurable: true, get: () => 600 });
  Object.defineProperty(HTMLElement.prototype, 'clientWidth', { configurable: true, get: () => 1200 });
});

beforeEach(() => {
  onEditCell.mockReset();
  onAddItem.mockReset();
  onOpenItem.mockReset();
  onMoveItem.mockReset();
  columnActions.onHide.mockReset();
});

function renderGrid({
  items = ITEMS,
  canEdit = true,
  reorderable = true,
}: { items?: ListItem[]; canEdit?: boolean; reorderable?: boolean } = {}) {
  render(
    <ListGrid
      label="Launch plan"
      columns={COLUMNS}
      items={items}
      canEdit={canEdit}
      reorderable={reorderable}
      canAdd
      emptyText="No items yet."
      people={PEOPLE}
      columnActions={columnActions}
      openItemId={null}
      onEditCell={onEditCell}
      onAddItem={onAddItem}
      onOpenItem={onOpenItem}
      onMoveItem={onMoveItem}
    />,
  );
  const grid = screen.getByRole('grid');
  grid.focus();
  return grid;
}

const press = (target: HTMLElement, key: string, options: Partial<KeyboardEventInit> = {}) =>
  fireEvent.keyDown(target, { key, ...options });

function activeSpot(grid: HTMLElement) {
  const cell = document.getElementById(grid.getAttribute('aria-activedescendant') ?? '');
  if (!cell) return null;
  if (cell.getAttribute('role') === 'columnheader') return { item: 'header', column: cell.textContent };
  return {
    item: cell.closest<HTMLElement>('[role="row"]')?.dataset.itemId,
    column: cell.dataset.column,
  };
}

const editor = () => screen.getByTestId('cell-editor');

describe('ListGrid keyboard navigation', () => {
  it('moves with the arrows, Home and End, and Tab wraps to the next row', () => {
    const grid = renderGrid();
    expect(activeSpot(grid)).toEqual({ item: 'i-1', column: 'Title' });

    press(grid, 'ArrowRight');
    expect(activeSpot(grid)).toEqual({ item: 'i-1', column: 'Points' });
    press(grid, 'ArrowDown');
    expect(activeSpot(grid)).toEqual({ item: 'i-2', column: 'Points' });
    press(grid, 'End');
    expect(activeSpot(grid)).toEqual({ item: 'i-2', column: 'Link' });
    press(grid, 'Tab');
    expect(activeSpot(grid)).toEqual({ item: 'i-3', column: 'Title' });
    press(grid, 'Tab', { shiftKey: true });
    expect(activeSpot(grid)).toEqual({ item: 'i-2', column: 'Link' });
    press(grid, 'Home');
    expect(activeSpot(grid)).toEqual({ item: 'i-2', column: 'Title' });
    press(grid, 'ArrowDown');
    press(grid, 'ArrowDown');
    expect(activeSpot(grid)).toEqual({ item: 'i-3', column: 'Title' });
  });

  it('reaches the column headers above the first row and opens a header menu with Enter', () => {
    const grid = renderGrid();
    press(grid, 'ArrowUp');
    expect(activeSpot(grid)).toEqual({ item: 'header', column: 'Title' });
    press(grid, 'Enter');
    fireEvent.click(screen.getByTestId('list-column-hide'));
    expect(columnActions.onHide).toHaveBeenCalledWith(COLUMNS[0]);
  });

  it('lets Tab leave the grid from its last cell', () => {
    const grid = renderGrid();
    press(grid, 'End', { ctrlKey: true });
    expect(activeSpot(grid)).toEqual({ item: 'i-3', column: 'Link' });
    expect(press(grid, 'Tab')).toBe(true);
  });

  it('edits with Enter, commits with Enter and moves down', () => {
    const grid = renderGrid();
    press(grid, 'Enter');
    expect(editor()).toHaveValue('First');
    fireEvent.change(editor(), { target: { value: 'Renamed' } });
    press(editor(), 'Enter');
    expect(onEditCell).toHaveBeenCalledWith('i-1', 'c-text', 'Renamed');
    expect(screen.queryByTestId('cell-editor')).toBeNull();
    expect(activeSpot(grid)).toEqual({ item: 'i-2', column: 'Title' });
    expect(grid).toHaveFocus();
  });

  it('cancels an edit with Escape', () => {
    const grid = renderGrid();
    press(grid, 'Enter');
    fireEvent.change(editor(), { target: { value: 'Never mind' } });
    press(editor(), 'Escape');
    expect(onEditCell).not.toHaveBeenCalled();
    expect(screen.queryByTestId('cell-editor')).toBeNull();
    expect(grid).toHaveFocus();
  });

  it('starts an edit with the character typed and Tab commits it moving right', () => {
    const grid = renderGrid();
    press(grid, 'ArrowDown');
    press(grid, 'x');
    expect(editor()).toHaveValue('x');
    press(editor(), 'Tab');
    expect(onEditCell).toHaveBeenCalledWith('i-2', 'c-text', 'x');
    expect(activeSpot(grid)).toEqual({ item: 'i-2', column: 'Points' });
  });

  it('clears a cell with Delete and leaves an empty one alone', () => {
    const grid = renderGrid();
    press(grid, 'Delete');
    expect(onEditCell).toHaveBeenCalledWith('i-1', 'c-text', null);
    press(grid, 'ArrowRight');
    press(grid, 'Backspace');
    expect(onEditCell).toHaveBeenCalledTimes(1);
  });

  it('moves a row with Alt and the arrows, and opens it with Shift+Enter', () => {
    const grid = renderGrid();
    press(grid, 'ArrowDown', { altKey: true });
    expect(onMoveItem).toHaveBeenCalledWith('i-1', 'i-2');
    press(grid, 'ArrowDown');
    press(grid, 'ArrowUp', { altKey: true });
    expect(onMoveItem).toHaveBeenLastCalledWith('i-2', null);
    press(grid, 'Enter', { shiftKey: true });
    expect(onOpenItem).toHaveBeenCalledWith('i-2');
  });

  it('keeps rows in place while the view sorts or filters them', () => {
    const grid = renderGrid({ reorderable: false });
    press(grid, 'ArrowDown', { altKey: true });
    expect(onMoveItem).not.toHaveBeenCalled();
    expect(screen.queryByTestId('list-row-grip')).toBeNull();
  });

  it('does not edit a read-only list', () => {
    const grid = renderGrid({ canEdit: false });
    press(grid, 'Enter');
    press(grid, 'a');
    press(grid, 'Delete');
    expect(screen.queryByTestId('cell-editor')).toBeNull();
    expect(onEditCell).not.toHaveBeenCalled();
    expect(screen.queryByTestId('list-add-item')).toBeNull();
  });
});

describe('ListGrid cell editors', () => {
  const goTo = (grid: HTMLElement, steps: number) => {
    for (let i = 0; i < steps; i += 1) press(grid, 'ArrowRight');
  };

  it('parses a number and refuses text that is not one', () => {
    const grid = renderGrid();
    goTo(grid, 1);
    press(grid, '4');
    fireEvent.change(editor(), { target: { value: 'four' } });
    press(editor(), 'Enter');
    expect(screen.getByRole('alert')).toHaveTextContent('Enter a number');
    expect(onEditCell).not.toHaveBeenCalled();
    fireEvent.change(editor(), { target: { value: '1,250.5' } });
    press(editor(), 'Enter');
    expect(onEditCell).toHaveBeenCalledWith('i-1', 'c-num', 1250.5);
  });

  it('picks a choice in a select', () => {
    const grid = renderGrid();
    goTo(grid, 2);
    press(grid, 'Enter');
    expect(editor()).toHaveValue('s-todo');
    fireEvent.change(editor(), { target: { value: 's-done' } });
    expect(onEditCell).toHaveBeenCalledWith('i-1', 'c-status', 's-done');
  });

  it('toggles several choices in a multi-select and commits with Enter', () => {
    const grid = renderGrid();
    goTo(grid, 3);
    press(grid, 'Enter');
    const listbox = editor();
    expect(listbox).toHaveAttribute('aria-label', 'Tags');
    press(listbox, ' ');
    press(listbox, 'ArrowDown');
    press(listbox, ' ');
    expect(screen.getAllByTestId('cell-editor-choice').map((o) => o.getAttribute('aria-selected'))).toEqual([
      'true',
      'true',
    ]);
    press(listbox, 'Enter');
    expect(onEditCell).toHaveBeenCalledWith('i-1', 'c-tags', ['t-a', 't-b']);
  });

  it('finds a member by what was typed and picks them with Enter', () => {
    const grid = renderGrid();
    goTo(grid, 4);
    press(grid, 'b');
    expect(editor()).toHaveValue('b');
    const options = screen.getAllByTestId('cell-editor-person');
    expect(options).toHaveLength(2);
    expect(options[0]).toHaveTextContent('Bob');
    expect(options[1]).toHaveTextContent('Nobody');
    press(editor(), 'Enter');
    expect(onEditCell).toHaveBeenCalledWith('i-1', 'c-owner', 'u-bob');
  });

  it('takes a date from the date picker', () => {
    const grid = renderGrid();
    goTo(grid, 5);
    press(grid, 'Enter');
    expect(editor()).toHaveAttribute('type', 'date');
    fireEvent.change(editor(), { target: { value: '2026-10-24' } });
    press(editor(), 'Enter');
    expect(onEditCell).toHaveBeenCalledWith('i-1', 'c-due', '2026-10-24');
  });

  it('toggles a checkbox with Enter and with a click', () => {
    const grid = renderGrid();
    goTo(grid, 6);
    press(grid, 'Enter');
    expect(onEditCell).toHaveBeenCalledWith('i-1', 'c-ok', true);
    const [, second] = screen.getAllByRole('gridcell').filter((cell) => cell.dataset.column === 'Finished');
    fireEvent.click(second);
    expect(onEditCell).toHaveBeenLastCalledWith('i-2', 'c-ok', true);
  });

  it('turns a bare domain into a link', () => {
    const grid = renderGrid();
    goTo(grid, 7);
    press(grid, 'e');
    fireEvent.change(editor(), { target: { value: 'example.com/bug/1' } });
    press(editor(), 'Enter');
    expect(onEditCell).toHaveBeenCalledWith('i-1', 'c-link', 'https://example.com/bug/1');
  });

  it('does not write a cell that did not change', () => {
    const grid = renderGrid();
    press(grid, 'Enter');
    press(editor(), 'Enter');
    expect(onEditCell).not.toHaveBeenCalled();
  });
});

describe('ListGrid virtualization', () => {
  it('renders a window of a 10,000 item list, not every row', () => {
    const many = Array.from({ length: 10_000 }, (_, i) => item(`i-${i}`, String(i).padStart(5, '0')));
    const grid = renderGrid({ items: many });
    expect(grid).toHaveAttribute('aria-rowcount', '10001');
    const rendered = screen.getAllByTestId('list-row').length;
    expect(rendered).toBeGreaterThan(10);
    expect(rendered).toBeLessThan(60);
  });
});
