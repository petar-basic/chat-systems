import { describe, it, expect, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { PollDialog } from './PollDialog';
import { moveRow } from './pollCommand';

const post = vi.fn();
vi.mock('@/shared/hooks/useCurrentApi', async () => {
  const { typedApiMock } = await import('@/test/typedApiMock');
  return { getApiForInstance: () => typedApiMock({ post: (...args) => post(...args) }) };
});

function renderDialog(threadParentId?: string) {
  const onClose = vi.fn();
  render(
    <QueryClientProvider client={new QueryClient()}>
      <PollDialog target={{ channelId: 'c-1', threadParentId }} onClose={onClose} />
    </QueryClientProvider>,
  );
  return onClose;
}

const type = (testId: string, value: string) =>
  fireEvent.change(screen.getByTestId(testId), { target: { value } });

describe('PollDialog', () => {
  beforeEach(() => post.mockReset());

  it('creates the poll with the options in the order shown', async () => {
    post.mockResolvedValue({ id: 'm-1', channel_id: 'c-1', thread_parent_id: 't-1' });
    const onClose = renderDialog('t-1');
    type('poll-dialog-question', 'Lunch?');
    type('poll-dialog-option-0', 'Thursday');
    type('poll-dialog-option-1', 'Friday');
    fireEvent.click(screen.getByTestId('poll-dialog-add-option'));
    type('poll-dialog-option-2', 'Never');
    fireEvent.keyDown(screen.getByTestId('poll-dialog-option-2'), { key: 'ArrowUp', altKey: true });
    fireEvent.click(screen.getByText('Any number'));
    fireEvent.click(screen.getByTestId('poll-dialog-anonymous'));
    fireEvent.click(screen.getByTestId('poll-dialog-submit'));

    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(post).toHaveBeenCalledWith('/channels/{ch_id}/polls', {
      question: 'Lunch?',
      options: ['Thursday', 'Never', 'Friday'],
      multiple_choice: true,
      anonymous: true,
      closes_at: null,
      thread_parent_id: 't-1',
    });
  });

  it('asks for a question and two options before sending anything', () => {
    renderDialog();
    fireEvent.click(screen.getByTestId('poll-dialog-submit'));
    expect(screen.getByText('Ask a question.')).toBeInTheDocument();
    type('poll-dialog-question', 'Lunch?');
    type('poll-dialog-option-0', 'Thursday');
    fireEvent.click(screen.getByTestId('poll-dialog-submit'));
    expect(screen.getByText('Give at least 2 options.')).toBeInTheDocument();
    expect(post).not.toHaveBeenCalled();
  });

  it('keeps at least two option rows and at most ten', () => {
    renderDialog();
    expect(screen.getByLabelText('Remove option 1')).toBeDisabled();
    for (let i = 0; i < 8; i++) fireEvent.click(screen.getByTestId('poll-dialog-add-option'));
    expect(screen.getAllByTestId('poll-dialog-option')).toHaveLength(10);
    expect(screen.queryByTestId('poll-dialog-add-option')).toBeNull();
    fireEvent.click(screen.getByLabelText('Remove option 10'));
    expect(screen.getAllByTestId('poll-dialog-option')).toHaveLength(9);
  });

  it('shows what the server refused', async () => {
    post.mockRejectedValue(new Error('"Yes" is listed twice'));
    const onClose = renderDialog();
    type('poll-dialog-question', 'Q');
    type('poll-dialog-option-0', 'Yes');
    type('poll-dialog-option-1', 'yes');
    fireEvent.click(screen.getByTestId('poll-dialog-submit'));
    expect(await screen.findByText('"Yes" is listed twice')).toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
  });
});

describe('moveRow', () => {
  it('moves one row and ignores a move off either end', () => {
    expect(moveRow(['a', 'b', 'c'], 2, 0)).toEqual(['c', 'a', 'b']);
    expect(moveRow(['a', 'b', 'c'], 0, -1)).toEqual(['a', 'b', 'c']);
    expect(moveRow(['a', 'b', 'c'], 2, 3)).toEqual(['a', 'b', 'c']);
  });
});
