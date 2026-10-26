import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import ChannelHeader from './ChannelHeader';
import type { Channel } from '@/stores/workspace';

const moderation = { canModerate: false };

vi.mock('react-router', () => ({ useNavigate: () => vi.fn() }));
vi.mock('@/features/channel/hooks/useChannels', () => ({
  useChannelMembers: () => ({ data: [{ user_id: 'u-1' }, { user_id: 'u-2' }, { user_id: 'u-3' }] }),
  useChannelPins: () => ({ data: [{ id: 'm-1' }, { id: 'm-2' }] }),
}));
vi.mock('@/features/workspace/hooks/useHooks', () => ({ useHookedChannels: () => ({ data: new Set() }) }));
vi.mock('./hooks/useChannelModeration', () => ({ useChannelModeration: () => moderation }));
vi.mock('@/features/huddle/components/HuddleBar', () => ({ HuddleBar: () => null }));
vi.mock('@/features/notifications/NotificationLevelMenu', () => ({
  NotificationLevelButton: () => null,
  NotificationLevelMenu: () => null,
}));
vi.mock('./ChannelSettingsModal', () => ({ default: () => null }));

const channel = {
  id: 'ch-1',
  workspace_id: 'ws-1',
  name: 'design',
  channel_type: 'public',
  topic: null,
} as unknown as Channel;

function renderHeader(onToggleChannelMembers = vi.fn()) {
  render(
    <ChannelHeader
      channel={channel}
      showSearch={false}
      showPins={false}
      showFiles={false}
      showChannelMembers={false}
      onToggleSearch={vi.fn()}
      onTogglePins={vi.fn()}
      onToggleFiles={vi.fn()}
      onToggleChannelMembers={onToggleChannelMembers}
    />,
  );
}

describe('ChannelHeader', () => {
  beforeEach(() => {
    moderation.canModerate = false;
  });

  it('has one members control, showing the count', () => {
    const onToggle = vi.fn();
    renderHeader(onToggle);
    const members = screen.getAllByRole('button', { name: /members/i });
    expect(members).toHaveLength(1);
    expect(members[0]).toHaveTextContent('3');
    fireEvent.click(members[0]);
    expect(onToggle).toHaveBeenCalledOnce();
  });

  it('shows the pin count on the pinned messages button', () => {
    renderHeader();
    expect(screen.getByRole('button', { name: 'Pinned messages' })).toHaveTextContent('2');
  });

  it('keeps channel settings in the more menu, for moderators only', () => {
    renderHeader();
    fireEvent.click(screen.getByRole('button', { name: 'More channel actions' }));
    expect(screen.queryByText('Channel settings')).not.toBeInTheDocument();
  });

  it('offers channel settings to a moderator', () => {
    moderation.canModerate = true;
    renderHeader();
    fireEvent.click(screen.getByRole('button', { name: 'More channel actions' }));
    expect(screen.getByTestId('channel-settings-open')).toHaveTextContent('Channel settings');
  });
});
