import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import i18n from '@/lib/i18n';
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

describe('ChannelArchivedBar in Japanese', () => {
  const hostile = '<img src=x onerror="alert(1)"> {{name}}';
  beforeEach(async () => {
    await i18n.changeLanguage('ja');
  });
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('says the channel is archived, naming it as text', () => {
    render(<ChannelArchivedBar channel={{ ...channel, name: hostile }} canUnarchive={false} />);
    expect(screen.getByTestId('channel-archived').textContent).toBe(
      `#${hostile} はアーカイブされています。過去の履歴は閲覧できますが、新しい投稿はできません。`,
    );
    expect(screen.getByText(`#${hostile}`)).toHaveClass('font-semibold');
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  it('offers to unarchive', () => {
    render(<ChannelArchivedBar channel={channel} canUnarchive />);
    expect(screen.getByTestId('channel-unarchive')).toHaveTextContent('アーカイブを解除');
  });
});
