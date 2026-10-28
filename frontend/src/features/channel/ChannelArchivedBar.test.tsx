import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ChannelArchivedBar } from './ChannelArchivedBar';
import type { Channel } from '@/stores/workspace';

const unarchive = vi.fn();

vi.mock('./hooks/useChannels', () => ({
  useUnarchiveChannel: () => ({ mutate: unarchive, isPending: false }),
}));

const channel = { id: 'c1', workspace_id: 'w1', name: 'launch', is_archived: true } as Channel;

describe('ChannelArchivedBar', () => {
  beforeEach(() => {
    unarchive.mockReset();
  });

  it('says the channel is archived and offers nothing to a member', () => {
    render(<ChannelArchivedBar channel={channel} canUnarchive={false} />);

    expect(screen.getByTestId('channel-archived')).toHaveTextContent('#launch is archived');
    expect(screen.queryByTestId('channel-unarchive')).not.toBeInTheDocument();
  });

  it('lets an admin bring it back', () => {
    render(<ChannelArchivedBar channel={channel} canUnarchive />);

    fireEvent.click(screen.getByTestId('channel-unarchive'));

    expect(unarchive).toHaveBeenCalledWith('c1', expect.anything());
  });
});
