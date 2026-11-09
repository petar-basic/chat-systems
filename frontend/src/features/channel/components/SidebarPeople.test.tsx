import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import i18n from '@/lib/i18n';
import { useUserCache } from '@/stores/users';
import { MemberRow } from './SidebarPeople';

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
