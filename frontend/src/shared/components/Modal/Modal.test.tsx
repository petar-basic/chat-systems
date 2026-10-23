import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { Modal } from './Modal';
import { Button } from '../Button/Button';

const renderModal = (onClose = vi.fn(), onSubmit?: () => void) => {
  const opener = document.createElement('button');
  document.body.appendChild(opener);
  opener.focus();
  const view = render(
    <Modal
      title="Create channel"
      description="Channels are where work happens"
      onClose={onClose}
      onSubmit={onSubmit}
      dismissLabel="Cancel"
      actions={
        <Button type="submit" variant="primary">
          Create
        </Button>
      }
    >
      <input aria-label="Name" data-autofocus />
    </Modal>,
  );
  return { onClose, opener, ...view };
};

describe('Modal', () => {
  it('keeps the title and the buttons outside the part that scrolls', () => {
    renderModal();
    const dialog = screen.getByRole('dialog', { name: 'Create channel' });
    const body = dialog.querySelector('[data-modal-body]');
    expect(body).toContainElement(screen.getByLabelText('Name'));
    expect(body).not.toContainElement(screen.getByRole('heading', { name: 'Create channel' }));
    expect(body).not.toContainElement(screen.getByRole('button', { name: 'Create' }));
    expect(screen.getByText('Channels are where work happens')).toBeInTheDocument();
  });

  it('closes from the X, Cancel, Escape and the backdrop', () => {
    const { onClose } = renderModal();
    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    fireEvent.keyDown(document, { key: 'Escape' });
    fireEvent.mouseDown(screen.getByRole('dialog').parentElement!);
    expect(onClose).toHaveBeenCalledTimes(4);
  });

  it('does not close on a press inside the dialog', () => {
    const { onClose } = renderModal();
    fireEvent.mouseDown(screen.getByLabelText('Name'));
    expect(onClose).not.toHaveBeenCalled();
  });

  it('submits the form from the footer', () => {
    const onSubmit = vi.fn((e: Event) => e.preventDefault());
    renderModal(vi.fn(), onSubmit as never);
    fireEvent.click(screen.getByRole('button', { name: 'Create' }));
    expect(onSubmit).toHaveBeenCalledTimes(1);
  });

  it('moves focus in and gives it back on close', () => {
    const { opener, unmount } = renderModal();
    expect(screen.getByLabelText('Name')).toHaveFocus();
    unmount();
    expect(document.activeElement).toBe(opener);
  });
});
