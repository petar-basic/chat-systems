import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import i18n from '@/lib/i18n';
import { useHuddleStore } from '@/stores/huddle';
import { HuddleBar } from './HuddleBar';

let activeHuddleChannels = new Map<string, { huddleId: string }>();

vi.mock('@/stores/workspace', () => ({
  useWorkspaceStore: (select: (state: unknown) => unknown) =>
    select({
      currentWorkspace: { id: 'ws-1', instanceUrl: 'http://localhost:3000' },
      currentUserId: 'u-1',
      activeHuddleChannels,
    }),
}));
vi.mock('@/features/huddle/hooks/useHuddle', () => ({
  useStartHuddle: () => ({ isPending: false, mutateAsync: vi.fn() }),
}));

describe('HuddleBar', () => {
  beforeEach(() => {
    activeHuddleChannels = new Map();
    useHuddleStore.setState({ active: null });
  });

  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('offers to start a huddle, or to join the one going on, in Japanese', async () => {
    await i18n.changeLanguage('ja');
    const { unmount } = render(<HuddleBar channelId="ch-1" />);
    expect(screen.getByRole('button', { name: '通話を開始' })).toHaveAttribute('data-tooltip', '通話を開始');
    unmount();

    activeHuddleChannels = new Map([['ch-1', { huddleId: 'h-1' }]]);
    render(<HuddleBar channelId="ch-1" />);
    expect(screen.getByRole('button', { name: '通話に参加' })).toBeInTheDocument();
  });
});
