import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { AllUnreadsView } from './AllUnreadsView';

const mutate = vi.fn();
const group = {
  channel_id: 'ch-1',
  channel_name: 'design',
  channel_type: 'public' as const,
  muted: false,
  unread_count: 2,
  mention_count: 0,
  last_activity_at: '2026-10-21T10:00:00Z',
  messages: [
    {
      id: 'm1',
      channel_id: 'ch-1',
      user_id: 'u2',
      client_message_id: null,
      content: 'first new message',
      metadata: {},
      thread_parent_id: null,
      reply_count: 0,
      is_pinned: false,
      created_at: '2026-10-21T09:00:00Z',
      updated_at: '2026-10-21T09:00:00Z',
      deleted_at: null,
    },
  ],
};

vi.mock('@/features/unreads/hooks/useUnreads', () => ({
  useUnreads: () => ({
    data: { pages: [{ data: [group], next_offset: null }] },
    isLoading: false,
    isError: false,
    isSuccess: true,
    hasNextPage: false,
  }),
  useMarkGroupRead: () => ({ mutate, isPending: false }),
}));

const renderView = () =>
  render(
    <AllUnreadsView
      workspaceId="ws-1"
      currentUserId="u1"
      conversations={[]}
      onOpenChannel={vi.fn()}
      onOpenConversation={vi.fn()}
    />,
  );

describe('AllUnreadsView', () => {
  beforeEach(() => mutate.mockClear());

  it('shows each channel with its new messages', () => {
    renderView();
    expect(screen.getByTestId('all-unreads-group')).toHaveTextContent('design');
    expect(screen.getByText('first new message')).toBeInTheDocument();
  });

  it('marks a channel read from its button', () => {
    renderView();
    fireEvent.click(screen.getByTestId('all-unreads-mark-read'));
    expect(mutate).toHaveBeenCalledWith(group);
  });

  it('marks the focused channel read on Escape', () => {
    renderView();
    const section = screen.getByTestId('all-unreads-group');
    section.focus();
    fireEvent.keyDown(section, { key: 'Escape' });
    expect(mutate).toHaveBeenCalledWith(group);
  });
});
