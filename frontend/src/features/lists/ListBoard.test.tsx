import { describe, it, expect, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/react';
import { ListBoard } from './ListBoard';
import { peopleFrom } from './people';
import type { ListColumn, ListItem } from './listModel';

const column = (id: string, name: string, kind: ListColumn['kind'], options: ListColumn['options'] = {}) =>
  ({ id, list_id: 'l-1', position: 0, name, kind, options, hidden: false }) satisfies ListColumn;

const TITLE = column('c-title', 'Task', 'text');
const STATUS = column('c-status', 'Status', 'select', {
  choices: [
    { id: 's-todo', label: 'To do', color: 'gray' },
    { id: 's-done', label: 'Done', color: 'green' },
  ],
});
const POINTS = column('c-points', 'Points', 'number');

const item = (id: string, values: ListItem['values']): ListItem => ({
  id,
  list_id: 'l-1',
  rank: id,
  values,
  created_by: 'u-ana',
  created_at: '2026-10-01T00:00:00Z',
  updated_at: '2026-10-01T00:00:00Z',
});

const ITEMS = [
  item('a', { 'c-title': 'Fix login', 'c-status': 's-todo', 'c-points': 3 }),
  item('b', { 'c-title': 'Write docs', 'c-status': 's-todo' }),
  item('c', { 'c-title': 'Ship it', 'c-status': 's-done' }),
  item('d', { 'c-status': 'gone' }),
];

const onOpenItem = vi.fn<(itemId: string) => void>();
const onSetGroup = vi.fn<(itemId: string, choiceId: string | null) => void>();
const onAddItem = vi.fn<(choiceId: string | null) => void>();

beforeEach(() => {
  onOpenItem.mockReset();
  onSetGroup.mockReset();
  onAddItem.mockReset();
});

function renderBoard({ items = ITEMS, canEdit = true, group = STATUS as ListColumn | null } = {}) {
  render(
    <ListBoard
      label="Launch plan"
      group={group}
      columns={[TITLE, STATUS, POINTS]}
      items={items}
      canEdit={canEdit}
      canAdd
      hasGroupable
      people={peopleFrom([], null)}
      openItemId={null}
      onOpenItem={onOpenItem}
      onSetGroup={onSetGroup}
      onAddItem={onAddItem}
    />,
  );
}

const lane = (name: string) => screen.getAllByTestId('list-lane').find((l) => l.dataset.lane === name)!;
const card = (title: string) =>
  screen.getAllByTestId('list-card').find((c) => c.textContent?.includes(title))!;
const titles = (name: string) =>
  within(lane(name))
    .queryAllByTestId('list-card-title')
    .map((t) => t.textContent);
const dataTransfer = () => ({ setData: vi.fn(), effectAllowed: '', dropEffect: '' });

describe('ListBoard', () => {
  it('puts each item in the lane of its choice, and anything else under No Status', () => {
    renderBoard();
    expect(screen.getAllByTestId('list-lane').map((l) => l.dataset.lane)).toEqual([
      'To do',
      'Done',
      'No Status',
    ]);
    expect(titles('To do')).toEqual(['Fix login', 'Write docs']);
    expect(titles('Done')).toEqual(['Ship it']);
    expect(titles('No Status')).toEqual(['Untitled']);
    expect(within(card('Fix login')).getByText('3')).toBeTruthy();
  });

  it('sets the value when a card is dragged to another lane', () => {
    renderBoard();
    fireEvent.dragStart(card('Write docs'), { dataTransfer: dataTransfer() });
    fireEvent.dragOver(lane('Done'), { dataTransfer: dataTransfer() });
    fireEvent.drop(lane('Done'), { dataTransfer: dataTransfer() });
    expect(onSetGroup).toHaveBeenCalledWith('b', 's-done');

    fireEvent.dragStart(card('Ship it'), { dataTransfer: dataTransfer() });
    fireEvent.drop(lane('No Status'), { dataTransfer: dataTransfer() });
    expect(onSetGroup).toHaveBeenLastCalledWith('c', null);
  });

  it('leaves a card dropped back on its own lane alone', () => {
    renderBoard();
    fireEvent.dragStart(card('Fix login'), { dataTransfer: dataTransfer() });
    fireEvent.drop(lane('To do'), { dataTransfer: dataTransfer() });
    expect(onSetGroup).not.toHaveBeenCalled();
  });

  it('moves a card between lanes with Alt and the arrows, and opens it with Enter', () => {
    renderBoard();
    fireEvent.keyDown(card('Fix login'), { key: 'ArrowRight', altKey: true });
    expect(onSetGroup).toHaveBeenCalledWith('a', 's-done');
    fireEvent.keyDown(card('Ship it'), { key: 'ArrowLeft', altKey: true });
    expect(onSetGroup).toHaveBeenLastCalledWith('c', 's-todo');
    fireEvent.click(card('Ship it'));
    expect(onOpenItem).toHaveBeenCalledWith('c');
  });

  it('walks cards with the arrows', () => {
    renderBoard();
    card('Fix login').focus();
    fireEvent.keyDown(card('Fix login'), { key: 'ArrowDown' });
    expect(document.activeElement).toBe(card('Write docs'));
    fireEvent.keyDown(card('Write docs'), { key: 'ArrowRight' });
    expect(document.activeElement).toBe(card('Ship it'));
  });

  it('adds an item straight into a lane', () => {
    renderBoard();
    fireEvent.click(within(lane('Done')).getByTestId('list-lane-add'));
    expect(onAddItem).toHaveBeenCalledWith('s-done');
  });

  it('cannot be dragged or changed when read-only', () => {
    renderBoard({ canEdit: false });
    expect(card('Fix login').getAttribute('draggable')).toBe('false');
    fireEvent.keyDown(card('Fix login'), { key: 'ArrowRight', altKey: true });
    expect(onSetGroup).not.toHaveBeenCalled();
    expect(screen.queryByTestId('list-lane-add')).toBeNull();
  });

  it('shows a long lane a page at a time', () => {
    const many = Array.from({ length: 120 }, (_, i) =>
      item(`m${String(i).padStart(3, '0')}`, { 'c-title': `Task ${i}`, 'c-status': 's-todo' }),
    );
    renderBoard({ items: many });
    expect(within(lane('To do')).getAllByTestId('list-card')).toHaveLength(50);
    fireEvent.click(within(lane('To do')).getByTestId('list-lane-more'));
    expect(within(lane('To do')).getAllByTestId('list-card')).toHaveLength(100);
  });

  it('asks for a column to group by when it has none', () => {
    renderBoard({ group: null });
    expect(screen.getByTestId('list-board-ungrouped').textContent).toMatch(/Pick a single-select column/);
  });
});
