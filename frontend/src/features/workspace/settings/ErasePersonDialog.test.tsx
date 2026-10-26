import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { ErasePersonDialog } from './ErasePersonDialog';

function renderDialog() {
  const onErase = vi.fn().mockResolvedValue(undefined);
  const onClose = vi.fn();
  render(<ErasePersonDialog name="Bob Smith" email="bob@dev.local" onErase={onErase} onClose={onClose} />);
  return { onErase, onClose };
}

describe('ErasePersonDialog', () => {
  it('names what goes', () => {
    renderDialog();
    const list = screen.getByTestId('erase-person-consequences');
    expect(list).toHaveTextContent('bob@dev.local');
    expect(list).toHaveTextContent('signed out everywhere');
    expect(list).toHaveTextContent('messages stay readable');
  });

  it('erases only once their name is typed', async () => {
    const { onErase, onClose } = renderDialog();
    const confirm = screen.getByTestId('erase-person-dialog-confirm');
    expect(confirm).toBeDisabled();

    fireEvent.change(screen.getByLabelText('Type Bob Smith to confirm'), { target: { value: 'Bob' } });
    expect(confirm).toBeDisabled();

    fireEvent.change(screen.getByLabelText('Type Bob Smith to confirm'), { target: { value: 'Bob Smith' } });
    expect(confirm).toBeEnabled();
    fireEvent.click(confirm);

    await waitFor(() => expect(onErase).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
  });
});
