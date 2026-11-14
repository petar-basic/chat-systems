import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import i18n from '@/lib/i18n';
import { fireEvent, render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router';
import type { ReactNode } from 'react';
import { UserHoverCard } from './UserHoverCard';
import { useUserCache } from '@/stores/users';
import { useWorkspaceStore } from '@/stores/workspace';
import { useProfilePanel } from '@/stores/profilePanel';
import type { ProfileField } from '@/features/people';

vi.mock('@/shared/components/Popover/HoverCard', () => ({
  HoverCard: ({ content }: { content: ReactNode }) => content,
}));
vi.mock('@/features/instance', () => ({ useCurrentUser: () => ({ data: { id: 'self' } }) }));
vi.mock('@/features/messaging/hooks/useConversations', () => ({
  useOpenConversation: () => ({ mutateAsync: vi.fn() }),
}));
vi.mock('@/features/huddle', () => ({ useStartHuddle: () => ({ mutateAsync: vi.fn() }) }));

let fields: ProfileField[] = [];

vi.mock('@/features/people/hooks/useProfileFields', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/features/people/hooks/useProfileFields')>()),
  useProfileFields: () => ({ fields }),
}));

const field = (id: string, label: string, on_card: boolean): ProfileField => ({
  id,
  workspace_id: 'ws-1',
  label,
  kind: 'text',
  options: [],
  admins_only: false,
  on_card,
  position: 0,
  value_count: 0,
});

function person(extra: {
  title?: string | null;
  pronouns?: string | null;
  left?: boolean;
  profile_fields?: Record<string, string>;
}) {
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
    fields = [];
    useWorkspaceStore.setState({
      currentWorkspace: { id: 'ws-1', instanceUrl: 'http://localhost:3000' } as never,
    });
  });

  it('shows the filled on-card fields and opens the full profile', () => {
    fields = [field('dept', 'Department', true), field('office', 'Office', true), field('pet', 'Pet', false)];
    person({ profile_fields: { office: 'Belgrade', pet: 'Cat' } });
    renderCard();

    const card = screen.getByTestId('hover-card-fields');
    expect(card).toHaveTextContent('OfficeBelgrade');
    expect(card).not.toHaveTextContent('Department');
    expect(card).not.toHaveTextContent('Pet');

    fireEvent.click(screen.getByTestId('hover-card-profile'));
    expect(useProfilePanel.getState().userId).toBe('u-ana');
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

  it('says someone who left is no longer here', async () => {
    person({ left: true });
    await i18n.changeLanguage('ja');
    renderCard();
    expect(screen.getByTestId('hover-card-left').textContent).toBe('このワークスペースにはもういません');
  });
});
