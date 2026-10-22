import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, within } from '@testing-library/react';
import LaterPanel from './LaterPanel';

const update = vi.fn();
const remove = vi.fn();
const remind = vi.fn();
const items: Record<string, unknown[]> = {};
vi.mock('@/hooks/queries/useLater', () => ({
  useLater: (_ws: string, status: string) => ({ data: items[status] ?? [], isLoading: false }),
  useLaterActions: () => ({
    update: { mutate: update },
    remove: { mutate: remove },
    remind: { mutate: remind },
  }),
}));

const base = {
  workspace_id: 'ws',
  created_by: 'me',
  channel_id: 'ch',
  note: null,
  reminded_at: null,
  completed_at: null,
  created_at: '2026-10-22T10:00:00Z',
};

function renderPanel(onNavigateToMessage = vi.fn()) {
  render(
    <LaterPanel
      workspaceId="ws"
      channels={[{ id: 'ch', name: 'general' } as never]}
      conversations={[]}
      onClose={vi.fn()}
      onNavigateToMessage={onNavigateToMessage}
      onOpenConversation={vi.fn()}
    />,
  );
  return onNavigateToMessage;
}

describe('LaterPanel', () => {
  beforeEach(() => {
    update.mockClear();
    remove.mockClear();
    remind.mockClear();
    items.in_progress = [
      {
        ...base,
        id: 'saved',
        status: 'in_progress',
        text: null,
        remind_at: '2000-01-01T09:00:00Z',
        message: {
          id: 'm-1',
          channel_id: 'ch',
          author_id: 'ana',
          content: 'the runbook link',
          attachments: [],
          sent_at: base.created_at,
        },
      },
      {
        ...base,
        id: 'rem',
        status: 'in_progress',
        text: 'stand-up',
        remind_at: '2999-01-01T09:00:00Z',
        message: null,
      },
    ];
    items.completed = [];
  });

  it('marks what is overdue and what is still to come', () => {
    renderPanel();
    const [saved, reminder] = screen.getAllByTestId('later-row');
    expect(within(saved).getByTestId('later-due')).toHaveTextContent('Overdue');
    expect(within(reminder).getByTestId('later-due')).toHaveTextContent('Due');
    expect(reminder).toHaveTextContent('stand-up');
    expect(screen.getByTestId('later-tab-in_progress')).toHaveTextContent('In progress 2');
  });

  it('ticks an item off, archives it, and opens its message', () => {
    const navigate = renderPanel();
    const [saved] = screen.getAllByTestId('later-row');
    fireEvent.click(within(saved).getByTestId('later-complete'));
    expect(update).toHaveBeenCalledWith({ id: 'saved', status: 'completed' });
    fireEvent.click(within(saved).getByTestId('later-archive'));
    expect(update).toHaveBeenCalledWith({ id: 'saved', status: 'archived' });
    fireEvent.click(within(saved).getByTestId('later-content'));
    expect(navigate).toHaveBeenCalledWith('ch', 'm-1');
  });

  it('shows the empty state of each tab', () => {
    renderPanel();
    fireEvent.click(screen.getByTestId('later-tab-completed'));
    expect(screen.getByTestId('later-empty')).toHaveTextContent('Nothing completed yet');
  });
});
