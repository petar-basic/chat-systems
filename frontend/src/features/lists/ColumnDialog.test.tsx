import { describe, it, expect, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { ColumnDialog } from './ColumnDialog';
import type { ListColumn } from './listModel';
import type { useColumnActions } from './useLists';

const STATUS: ListColumn = {
  id: 'c-status',
  list_id: 'l-1',
  position: 0,
  name: 'Status',
  kind: 'select',
  options: {
    choices: [
      { id: 's-todo', label: 'To do', color: 'gray' },
      { id: 's-done', label: 'Done', color: 'green' },
    ],
  },
  hidden: false,
  due: false,
};

const add = vi.fn();
const update = vi.fn();
const preview = vi.fn();
const actions = {
  add: { mutateAsync: add },
  update: { mutateAsync: update },
  preview,
} as unknown as ReturnType<typeof useColumnActions>;

beforeEach(() => {
  add.mockReset();
  update.mockReset();
  preview.mockReset();
});

const type = (testId: string, value: string) =>
  fireEvent.change(screen.getByTestId(testId), { target: { value } });

describe('ColumnDialog', () => {
  it('adds a select column with its choices', async () => {
    add.mockResolvedValue({});
    const onClose = vi.fn();
    render(<ColumnDialog column={null} actions={actions} onClose={onClose} />);
    type('list-column-name', 'Priority');
    type('list-column-kind', 'select');
    fireEvent.click(screen.getByTestId('list-column-add-choice'));
    type('list-column-choice', 'High');
    fireEvent.click(screen.getByTestId('list-column-save'));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(add).toHaveBeenCalledWith({
      name: 'Priority',
      kind: 'select',
      options: { choices: [{ id: null, label: 'High', color: 'gray' }], number: undefined },
    });
  });

  it('renames without asking about cells', async () => {
    update.mockResolvedValue({});
    render(<ColumnDialog column={STATUS} actions={actions} onClose={vi.fn()} />);
    type('list-column-name', 'State');
    fireEvent.click(screen.getByTestId('list-column-save'));
    await waitFor(() =>
      expect(update).toHaveBeenCalledWith({
        columnId: 'c-status',
        body: { name: 'State', confirm_cleared: null },
      }),
    );
    expect(preview).not.toHaveBeenCalled();
  });

  it('names how many cells a kind change clears and sends that number to confirm', async () => {
    preview.mockResolvedValue({ converted: 3, cleared: 2 });
    update.mockResolvedValue({});
    const onClose = vi.fn();
    render(<ColumnDialog column={STATUS} actions={actions} onClose={onClose} />);
    type('list-column-kind', 'number');
    fireEvent.click(screen.getByTestId('list-column-save'));

    expect(await screen.findByTestId('list-column-clearing')).toHaveTextContent('clears 2 cells');
    expect(screen.getByTestId('list-column-save')).toHaveTextContent('Clear 2 cells and save');
    expect(update).not.toHaveBeenCalled();

    fireEvent.click(screen.getByTestId('list-column-save'));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(update).toHaveBeenCalledWith({
      columnId: 'c-status',
      body: {
        name: 'Status',
        kind: 'number',
        options: { choices: [], number: undefined },
        confirm_cleared: 2,
      },
    });
  });

  it('shows the server refusal and asks again', async () => {
    preview.mockResolvedValue({ converted: 0, cleared: 1 });
    update.mockRejectedValue(new Error('This change clears 4 cells'));
    render(<ColumnDialog column={STATUS} actions={actions} onClose={vi.fn()} />);
    type('list-column-kind', 'text');
    fireEvent.click(screen.getByTestId('list-column-save'));
    await screen.findByTestId('list-column-clearing');
    fireEvent.click(screen.getByTestId('list-column-save'));
    expect(await screen.findByText('This change clears 4 cells')).toBeInTheDocument();
    expect(screen.queryByTestId('list-column-clearing')).toBeNull();
  });
});
