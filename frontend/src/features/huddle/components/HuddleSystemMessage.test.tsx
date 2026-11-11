import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import i18n from '@/lib/i18n';
import { useUserCache } from '@/stores/users';
import { useWorkspaceStore } from '@/stores/workspace';
import { useHuddleStore } from '@/stores/huddle';
import { HuddleSystemMessage } from './HuddleSystemMessage';

const hostile = '<img src=x onerror="alert(1)"> {{name}}';

describe('HuddleSystemMessage in Japanese', () => {
  beforeEach(async () => {
    useUserCache.setState({ users: new Map([['u-x', { id: 'u-x', display_name: hostile } as never]]) });
    useWorkspaceStore.setState({
      currentWorkspace: { id: 'ws-1', instanceUrl: 'http://localhost:3000' } as never,
      currentUserId: 'u-me',
      activeHuddleChannels: new Map([['ch-1', { huddleId: 'h-1' }]]) as never,
    });
    useHuddleStore.setState({ active: null });
    await i18n.changeLanguage('ja');
  });
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('says who started the huddle, as text, and offers to join', () => {
    const { container } = render(<HuddleSystemMessage channelId="ch-1" huddleId="h-1" initiatorId="u-x" />);
    expect(container.textContent).toBe(`${hostile} さんが通話を開始しました参加`);
    expect(screen.getByText(hostile)).toHaveClass('font-medium');
    expect(screen.getByRole('button', { name: '参加' })).toBeInTheDocument();
    expect(document.querySelector('[onerror]')).toBeNull();
  });
});
