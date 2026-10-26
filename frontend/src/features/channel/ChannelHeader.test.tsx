import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import ChannelHeader from './ChannelHeader';
import type { Channel } from '@/stores/workspace';

const moderation = { canModerate: false };
let hooked = new Set<string>();

vi.mock('react-router', () => ({ useNavigate: () => vi.fn() }));
vi.mock('@/features/channel/hooks/useChannels', () => ({
  useChannelMembers: () => ({ data: [{ user_id: 'u-1' }, { user_id: 'u-2' }, { user_id: 'u-3' }] }),
  useChannelPins: () => ({ data: [{ id: 'm-1' }, { id: 'm-2' }] }),
}));
vi.mock('@/features/workspace/hooks/useHooks', () => ({ useHookedChannels: () => ({ data: hooked }) }));
vi.mock('./hooks/useChannelModeration', () => ({ useChannelModeration: () => moderation }));
vi.mock('@/features/huddle/components/HuddleBar', () => ({ HuddleBar: () => null }));
vi.mock('@/features/notifications/NotificationLevelMenu', () => ({
  NotificationLevelButton: () => null,
  NotificationLevelMenu: () => null,
}));
vi.mock('./details/ChannelDetailsModal', () => ({
  ChannelDetailsModal: ({ initialTab, canModerate }: { initialTab: string; canModerate: boolean }) => (
    <div data-qa="channel-details" data-tab={initialTab} data-moderator={String(canModerate)} />
  ),
}));

const channel = {
  id: 'ch-1',
  workspace_id: 'ws-1',
  name: 'design',
  channel_type: 'public',
  topic: null,
} as unknown as Channel;

function renderHeader(onToggleChannelMembers = vi.fn()) {
  return render(
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
    hooked = new Set();
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

  it('opens channel details from the more menu, for everyone in the channel', () => {
    for (const canModerate of [false, true]) {
      moderation.canModerate = canModerate;
      const { unmount } = renderHeader();
      fireEvent.click(screen.getByRole('button', { name: 'More channel actions' }));
      fireEvent.click(screen.getByTestId('channel-details-open'));
      const details = screen.getByTestId('channel-details');
      expect(details).toHaveAttribute('data-tab', 'about');
      expect(details).toHaveAttribute('data-moderator', String(canModerate));
      unmount();
    }
  });

  it('opens the integrations of a channel that forwards its messages from the indicator', () => {
    hooked = new Set(['ch-1']);
    renderHeader();
    fireEvent.click(screen.getByTestId('channel-integration-indicator'));
    expect(screen.getByTestId('channel-details')).toHaveAttribute('data-tab', 'integrations');
  });
});
