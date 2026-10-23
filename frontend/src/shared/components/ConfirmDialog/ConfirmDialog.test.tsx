import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { ConfirmDialog } from './ConfirmDialog';

const renderDialog = (onConfirm: () => Promise<void> | void, onClose = vi.fn()) => {
  render(
    <ConfirmDialog
      title="Delete message?"
      description="This can't be undone."
      confirmLabel="Delete"
      onConfirm={onConfirm}
      onClose={onClose}
    >
      <p>see you at the standup</p>
    </ConfirmDialog>,
  );
  return onClose;
};

describe('ConfirmDialog', () => {
  it('shows what goes and puts focus on the action, so Enter confirms', () => {
    renderDialog(vi.fn());
    expect(screen.getByText('see you at the standup')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Delete' })).toHaveFocus();
  });

  it('closes once the action is done', async () => {
    const onConfirm = vi.fn().mockResolvedValue(undefined);
    const onClose = renderDialog(onConfirm);
    fireEvent.click(screen.getByRole('button', { name: 'Delete' }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(onConfirm).toHaveBeenCalledTimes(1);
  });

  it('stays open and says why when the action fails', async () => {
    const onClose = renderDialog(vi.fn().mockRejectedValue(new Error('You can no longer delete it.')));
    fireEvent.click(screen.getByRole('button', { name: 'Delete' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('You can no longer delete it.');
    expect(onClose).not.toHaveBeenCalled();
  });

  it('cancels on Escape without acting', () => {
    const onConfirm = vi.fn();
    const onClose = renderDialog(onConfirm);
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(onClose).toHaveBeenCalled();
    expect(onConfirm).not.toHaveBeenCalled();
  });
});
