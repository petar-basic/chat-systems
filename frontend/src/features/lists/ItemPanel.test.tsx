import { describe, it, expect, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/react';
import { ItemPanel } from './ItemPanel';
import { peopleFrom } from './people';
import type { ListColumn, ListItem } from './listModel';
import type { WorkspaceMember } from '@/stores/workspace';

const column = (
  id: string,
  name: string,
  kind: ListColumn['kind'],
  options: ListColumn['options'] = {},
  hidden = false,
) => ({ id, list_id: 'l-1', position: 0, name, kind, options, hidden, due: false }) satisfies ListColumn;

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
  column('c-due', 'Due', 'date', {}, true),
  column('c-ok', 'Finished', 'checkbox'),
];

const ITEM: ListItem = {
  id: 'i-1',
  list_id: 'l-1',
  rank: 'V',
  values: { 'c-text': 'Fix login', 'c-tags': ['t-a'], 'c-due': '2026-10-30' },
  comment_count: 0,
  created_by: 'u-ana',
  created_at: '2026-10-01T00:00:00Z',
  updated_at: '2026-10-01T00:00:00Z',
};

const member = (user_id: string, display_name: string) =>
  ({ user_id, display_name, email: `${display_name}@x.dev`, role: 'member' }) as WorkspaceMember;
const PEOPLE = peopleFrom([member('u-ana', 'Ana'), member('u-bob', 'Bob')], null);

const onEdit = vi.fn<(columnId: string, value: unknown) => void>();
const onDelete = vi.fn();

beforeEach(() => {
  onEdit.mockReset();
  onDelete.mockReset();
});

function renderPanel(canEdit = true) {
  render(
    <ItemPanel
      item={ITEM}
      columns={COLUMNS}
      canEdit={canEdit}
      people={PEOPLE}
      onEdit={onEdit}
      onDelete={onDelete}
      onClose={vi.fn()}
    />,
  );
}

const field = (name: string) =>
  within(screen.getAllByTestId('item-field').find((el) => el.dataset.column === name)!);

describe('ItemPanel', () => {
  it('shows every field, the hidden ones too, under the item title', () => {
    renderPanel();
    expect(screen.getByRole('heading', { name: 'Fix login' })).toBeInTheDocument();
    expect(screen.getAllByTestId('item-field').map((el) => el.dataset.column)).toEqual(
      COLUMNS.map((c) => c.name),
    );
    expect(field('Due').getByTestId('item-field-input')).toHaveValue('2026-10-30');
  });

  it('saves text on blur and only when it changed', () => {
    renderPanel();
    const title = field('Title').getByTestId('item-field-input');
    fireEvent.blur(title);
    expect(onEdit).not.toHaveBeenCalled();
    fireEvent.change(title, { target: { value: 'Fix logout' } });
    fireEvent.blur(title);
    expect(onEdit).toHaveBeenCalledWith('c-text', 'Fix logout');
  });

  it('refuses a number it cannot read', () => {
    renderPanel();
    const points = field('Points').getByTestId('item-field-input');
    fireEvent.change(points, { target: { value: 'lots' } });
    fireEvent.keyDown(points, { key: 'Enter' });
    expect(screen.getByRole('alert')).toHaveTextContent('Enter a number');
    fireEvent.change(points, { target: { value: '8' } });
    fireEvent.keyDown(points, { key: 'Enter' });
    expect(onEdit).toHaveBeenCalledWith('c-num', 8);
  });

  it('edits selects, people, choices and checkboxes directly', () => {
    renderPanel();
    fireEvent.change(field('Status').getByTestId('item-field-select'), { target: { value: 's-done' } });
    expect(onEdit).toHaveBeenLastCalledWith('c-status', 's-done');
    fireEvent.change(field('Owner').getByTestId('item-field-person'), { target: { value: 'u-bob' } });
    expect(onEdit).toHaveBeenLastCalledWith('c-owner', 'u-bob');
    const [alpha, beta] = field('Tags').getAllByTestId('item-field-choice');
    fireEvent.click(beta);
    expect(onEdit).toHaveBeenLastCalledWith('c-tags', ['t-a', 't-b']);
    fireEvent.click(alpha);
    expect(onEdit).toHaveBeenLastCalledWith('c-tags', null);
    fireEvent.click(field('Finished').getByTestId('item-field-checkbox'));
    expect(onEdit).toHaveBeenLastCalledWith('c-ok', true);
  });

  it('deletes the item only after confirming', () => {
    renderPanel();
    fireEvent.click(screen.getByTestId('list-item-delete'));
    expect(onDelete).not.toHaveBeenCalled();
    fireEvent.click(
      within(screen.getByTestId('list-item-delete-confirm')).getByRole('button', { name: 'Delete item' }),
    );
    return vi.waitFor(() => expect(onDelete).toHaveBeenCalled());
  });

  it('shows values without controls when the list is read-only', () => {
    renderPanel(false);
    expect(screen.queryByTestId('item-field-input')).toBeNull();
    expect(screen.queryByTestId('list-item-delete')).toBeNull();
    expect(field('Tags').getByText('Alpha')).toBeInTheDocument();
  });
});
