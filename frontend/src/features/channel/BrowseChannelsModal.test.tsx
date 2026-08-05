import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import BrowseChannelsModal from './BrowseChannelsModal';
import type { BrowsableChannel } from '@/hooks/queries/useChannels';

const joinMutate = vi.fn();
const leaveMutate = vi.fn();
const channels: BrowsableChannel[] = [
  {
    id: 'ch-open',
    workspace_id: 'ws-1',
    name: 'design',
    channel_type: 'public',
    topic: 'pixels and pushback',
    description: null,
    is_default: false,
    created_at: '2026-01-01T00:00:00Z',
    member_count: 3,
    is_member: false,
  },
  {
    id: 'ch-joined',
    workspace_id: 'ws-1',
    name: 'general',
    channel_type: 'public',
    topic: null,
    description: null,
    is_default: true,
    created_at: '2026-01-01T00:00:00Z',
    member_count: 1,
    is_member: true,
  },
];

vi.mock('@/hooks/queries/useChannels', () => ({
  useBrowsableChannels: () => ({ data: channels, isLoading: false, isError: false }),
  useJoinChannel: () => ({ mutateAsync: joinMutate, isPending: false }),
  useLeaveChannel: () => ({ mutateAsync: leaveMutate, isPending: false }),
}));

function renderModal(overrides: Partial<Parameters<typeof BrowseChannelsModal>[0]> = {}) {
  const props = {
    workspaceId: 'ws-1',
    instanceUrl: 'http://localhost:8080',
    currentUserId: 'user-1',
    onClose: vi.fn(),
    onOpenChannel: vi.fn(),
    ...overrides,
  };
  render(<BrowseChannelsModal {...props} />);
  return props;
}

describe('BrowseChannelsModal', () => {
  beforeEach(() => {
    joinMutate.mockReset().mockResolvedValue(undefined);
    leaveMutate.mockReset().mockResolvedValue(undefined);
  });

  it('offers Join for channels the user is not in and Open/Leave for joined ones', () => {
    renderModal();

    const rows = screen.getAllByTestId('browse-channel-row');
    expect(rows).toHaveLength(2);
    expect(screen.getByTestId('browse-channel-join')).toBeInTheDocument();
    expect(screen.getByTestId('browse-channel-open')).toBeInTheDocument();
    expect(screen.getByTestId('browse-channel-leave')).toBeInTheDocument();
    expect(screen.getByText(/3 members/)).toBeInTheDocument();
    expect(screen.getByText(/1 member/)).toBeInTheDocument();
  });

  it('filters by name and topic', () => {
    renderModal();
    const search = screen.getByLabelText('Search channels');

    fireEvent.change(search, { target: { value: 'desi' } });
    expect(screen.getAllByTestId('browse-channel-row')).toHaveLength(1);

    fireEvent.change(search, { target: { value: 'pushback' } });
    expect(screen.getAllByTestId('browse-channel-row')).toHaveLength(1);

    fireEvent.change(search, { target: { value: 'nothing-matches' } });
    expect(screen.getByTestId('browse-channels-empty')).toBeInTheDocument();
  });

  it('joins a channel, then opens it and closes the modal', async () => {
    const props = renderModal();

    fireEvent.click(screen.getByTestId('browse-channel-join'));

    await waitFor(() => expect(joinMutate).toHaveBeenCalledWith('ch-open'));
    expect(props.onOpenChannel).toHaveBeenCalledWith('ch-open');
    expect(props.onClose).toHaveBeenCalled();
  });

  it('surfaces a failed join instead of closing', async () => {
    joinMutate.mockRejectedValue(new Error('Requires at least Member role'));
    const props = renderModal();

    fireEvent.click(screen.getByTestId('browse-channel-join'));

    await screen.findByText('Requires at least Member role');
    expect(props.onClose).not.toHaveBeenCalled();
  });

  it('leaves a joined channel without navigating away', async () => {
    const props = renderModal();

    fireEvent.click(screen.getByTestId('browse-channel-leave'));

    await waitFor(() => expect(leaveMutate).toHaveBeenCalledWith('ch-joined'));
    expect(props.onOpenChannel).not.toHaveBeenCalled();
    expect(props.onClose).not.toHaveBeenCalled();
  });
});
