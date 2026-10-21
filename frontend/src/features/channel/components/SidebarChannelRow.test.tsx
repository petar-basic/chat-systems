import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { SidebarChannelRow } from './SidebarChannelRow';
import type { Channel } from '@/stores/workspace';

const channel = { id: 'ch-1', name: 'design', channel_type: 'public' } as Channel;

describe('SidebarChannelRow', () => {
  it('moves a channel into a section from its menu', () => {
    const onMove = vi.fn();
    render(
      <SidebarChannelRow
        channel={channel}
        active={false}
        muted={false}
        unread={false}
        mentioned={false}
        unreadCount={undefined}
        mentionCount={undefined}
        onSelect={vi.fn()}
        onToggleMute={vi.fn()}
        moveTargets={[
          { id: null, name: 'Channels' },
          { id: 'sec-1', name: 'Projects' },
        ]}
        sectionId={null}
        onMove={onMove}
      />,
    );
    fireEvent.click(screen.getByTestId('sidebar-channel-move'));
    expect(screen.queryByRole('button', { name: 'Channels' })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Projects' }));
    expect(onMove).toHaveBeenCalledWith('ch-1', 'sec-1');
  });
});
