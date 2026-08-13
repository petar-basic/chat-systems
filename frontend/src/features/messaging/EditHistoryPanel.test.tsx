import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import EditHistoryPanel from './EditHistoryPanel';

const edits = [
  {
    id: 'e2',
    message_id: 'm1',
    previous_content: 'second',
    edited_by: 'u1',
    edited_at: '2026-01-02T00:00:00Z',
  },
  {
    id: 'e1',
    message_id: 'm1',
    previous_content: 'first',
    edited_by: 'u1',
    edited_at: '2026-01-01T00:00:00Z',
  },
];

let state: { data?: typeof edits; isLoading: boolean; error?: Error } = { data: edits, isLoading: false };

vi.mock('@/hooks/queries/useEditHistory', () => ({
  useEditHistory: () => state,
}));
vi.mock('@/hooks/queries/useAuth', () => ({
  useCurrentUser: () => ({ data: { id: 'u1' } }),
}));

describe('EditHistoryPanel', () => {
  it('shows the current text and every earlier version, newest first', () => {
    state = { data: edits, isLoading: false };
    render(<EditHistoryPanel messageId="m1" scope="channel" currentContent="third" onClose={vi.fn()} />);

    expect(screen.getByTestId('edit-history-current')).toHaveTextContent('third');
    const versions = screen.getAllByTestId('edit-history-version');
    expect(versions).toHaveLength(2);
    expect(versions[0]).toHaveTextContent('second');
    expect(versions[1]).toHaveTextContent('first');
  });

  it('says so when there is nothing behind the marker', () => {
    state = { data: [], isLoading: false };
    render(<EditHistoryPanel messageId="m1" scope="channel" currentContent="only" onClose={vi.fn()} />);
    expect(screen.getByText('No earlier versions.')).toBeInTheDocument();
  });

  it('surfaces a failed load rather than rendering an empty history', () => {
    state = { isLoading: false, error: new Error('Requires at least Admin role') };
    render(<EditHistoryPanel messageId="m1" scope="channel" currentContent="only" onClose={vi.fn()} />);
    expect(screen.getByText('Requires at least Admin role')).toBeInTheDocument();
    expect(screen.queryByTestId('edit-history-version')).toBeNull();
  });
});
