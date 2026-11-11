import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import i18n from '@/lib/i18n';
import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router';
import type { ReactNode } from 'react';
import { UserHoverCard } from './UserHoverCard';
import { useUserCache } from '@/stores/users';
import { useWorkspaceStore } from '@/stores/workspace';

vi.mock('@/shared/components/Popover/HoverCard', () => ({
  HoverCard: ({ content }: { content: ReactNode }) => content,
}));
vi.mock('@/features/instance', () => ({ useCurrentUser: () => ({ data: { id: 'self' } }) }));
vi.mock('@/features/messaging/hooks/useConversations', () => ({
  useOpenConversation: () => ({ mutateAsync: vi.fn() }),
}));
vi.mock('@/features/huddle', () => ({ useStartHuddle: () => ({ mutateAsync: vi.fn() }) }));

function person(extra: { title?: string | null; pronouns?: string | null; left?: boolean }) {
  useUserCache.setState({
    users: new Map([
      ['u-ana', { id: 'u-ana', email: 'ana@x.test', display_name: 'Ana', avatar_url: null, ...extra }],
    ]),
  });
}

function renderCard() {
  render(
    <MemoryRouter>
      <UserHoverCard userId="u-ana" name="Ana">
        <span>Ana</span>
      </UserHoverCard>
    </MemoryRouter>,
  );
}

describe('UserHoverCard', () => {
  beforeEach(() => {
    useWorkspaceStore.setState({
      currentWorkspace: { id: 'ws-1', instanceUrl: 'http://localhost:3000' } as never,
    });
  });

  it('shows the title under the name and the pronouns beside it', () => {
    person({ title: 'Head of Finance', pronouns: 'she/her' });
    renderCard();

    expect(screen.getByTestId('hover-card-title')).toHaveTextContent('Head of Finance');
    expect(screen.getByTestId('hover-card-pronouns')).toHaveTextContent('she/her');
  });

  it('leaves both out when the person has set neither', () => {
    person({ title: null, pronouns: '  ' });
    renderCard();

    expect(screen.queryByTestId('hover-card-title')).not.toBeInTheDocument();
    expect(screen.queryByTestId('hover-card-pronouns')).not.toBeInTheDocument();
  });

  it('says someone who left is no longer here and offers no message or huddle', () => {
    person({ left: true });
    renderCard();

    expect(screen.getByTestId('hover-card-left')).toHaveTextContent('No longer in this workspace');
    expect(screen.queryByText('Offline')).not.toBeInTheDocument();
    expect(screen.queryByTestId('hover-card-message')).not.toBeInTheDocument();
    expect(screen.queryByTestId('hover-card-huddle')).not.toBeInTheDocument();
  });
});

describe('UserHoverCard in Japanese', () => {
  beforeEach(() => {
    useWorkspaceStore.setState({
      currentWorkspace: { id: 'ws-1', instanceUrl: 'http://localhost:3000' } as never,
    });
  });

  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('names the presence and the buttons', async () => {
    person({ title: null, pronouns: null });
    await i18n.changeLanguage('ja');
    renderCard();
    expect(screen.getByText('オフライン')).toBeInTheDocument();
    expect(screen.getByTestId('hover-card-message')).toHaveTextContent('メッセージ');
    expect(screen.getByText('通話').closest('button')).toHaveAttribute('data-tooltip', '通話を開始');
  });
});
