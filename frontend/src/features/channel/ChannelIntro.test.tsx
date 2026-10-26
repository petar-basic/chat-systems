import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ChannelIntro } from './ChannelIntro';
import type { Channel } from '@/stores/workspace';

const channel = (over: Partial<Channel> = {}) =>
  ({
    id: 'ch-1',
    name: 'design',
    channel_type: 'public',
    is_default: false,
    description: null,
    topic: null,
    ...over,
  }) as Channel;

describe('ChannelIntro', () => {
  it('says what the channel is for', () => {
    render(<ChannelIntro channel={channel({ description: 'Mockups and reviews' })} />);
    expect(screen.getByText('This is the very beginning of #design')).toBeInTheDocument();
    expect(screen.getByTestId('channel-intro-purpose')).toHaveTextContent('Mockups and reviews');
  });

  it('explains #general when nobody described it', () => {
    render(<ChannelIntro channel={channel({ name: 'general', is_default: true })} />);
    expect(screen.getByTestId('channel-intro-purpose')).toHaveTextContent(
      'Everyone in the workspace is here',
    );
  });

  it('offers only what the viewer may do', () => {
    const onAddPeople = vi.fn();
    const onAddBookmark = vi.fn();
    const { rerender } = render(
      <ChannelIntro channel={channel()} onAddPeople={onAddPeople} onAddBookmark={onAddBookmark} />,
    );
    fireEvent.click(screen.getByTestId('channel-intro-add-people'));
    fireEvent.click(screen.getByTestId('channel-intro-add-bookmark'));
    expect(onAddPeople).toHaveBeenCalled();
    expect(onAddBookmark).toHaveBeenCalled();

    rerender(<ChannelIntro channel={channel()} />);
    expect(screen.queryByTestId('channel-intro-add-people')).toBeNull();
    expect(screen.queryByTestId('channel-intro-add-bookmark')).toBeNull();
  });
});
