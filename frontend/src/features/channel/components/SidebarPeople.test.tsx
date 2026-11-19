import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import i18n from '@/lib/i18n';
import { useUserCache } from '@/stores/users';
import { useDraftStore } from '@/stores/drafts';
import { ConversationRow, MemberRow } from './SidebarPeople';

const hostile = '<img src=x onerror="alert(1)"> {{name}}';

function person(status_text: string | null) {
  useUserCache.setState({
    users: new Map([
      ['u-ana', { id: 'u-ana', email: 'ana@x.test', display_name: hostile, avatar_url: null, status_text }],
    ]),
  });
}

describe('MemberRow', () => {
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('in Japanese offers a message to the person, with their status, as text', async () => {
    await i18n.changeLanguage('ja');
    person(null);
    const { unmount } = render(<MemberRow userId="u-ana" onOpenDm={vi.fn()} />);
    expect(screen.getByRole('button').getAttribute('data-tooltip')).toBe(`${hostile} にメッセージ`);
    unmount();

    person(`${hostile} status`);
    render(<MemberRow userId="u-ana" onOpenDm={vi.fn()} />);
    expect(screen.getByRole('button').getAttribute('data-tooltip')).toBe(
      `${hostile} にメッセージ — ${hostile} status`,
    );
    expect(document.querySelector('img')).toBeNull();
  });
});

describe('ConversationRow in Japanese', () => {
  afterEach(async () => {
    useDraftStore.setState({ drafts: {} });
    await i18n.changeLanguage('en');
  });

  it('names the pencil of a conversation holding a draft, beside a name shown as text', async () => {
    await i18n.changeLanguage('ja');
    person(null);
    useDraftStore.getState().setDraft('dm-1', 'half a thought');
    render(
      <ConversationRow
        conversation={
          { id: 'dm-1', kind: 'direct', participant_ids: ['u-me', 'u-ana'], departed: [] } as never
        }
        currentUserId="u-me"
        isActive={false}
        isUnread={false}
        onSelect={vi.fn()}
      />,
    );
    expect(screen.getByTestId('conversation-draft')).toHaveAttribute('aria-label', '下書き');
    expect(screen.getByTestId('conversation-row')).toHaveTextContent(hostile);
    expect(document.querySelector('[onerror]')).toBeNull();
  });
});
