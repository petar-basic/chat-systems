import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { act, render, screen } from '@testing-library/react';
import i18n from '@/lib/i18n';
import { globalEventBus } from '@/lib/globalEventBus';
import { useUserCache } from '@/stores/users';
import TypingIndicator from './TypingIndicator';

const people = ['Ana', 'Bea', 'Cid', 'Dan', 'Eva'];

function typing(userId: string) {
  act(() => {
    globalEventBus.emit({
      type: 'typing.indicator',
      channel_id: 'ch-1',
      user_id: userId,
      is_typing: true,
    } as never);
  });
}

describe('TypingIndicator', () => {
  beforeEach(() => {
    useUserCache.setState({
      users: new Map(
        people.map((name) => [
          name,
          { id: name, email: `${name}@x.test`, display_name: name, avatar_url: null },
        ]),
      ),
    });
  });

  it('names one or two people and counts everyone else', () => {
    render(<TypingIndicator channelId="ch-1" currentUserId="me" />);
    typing('Ana');
    expect(screen.getByTestId('typing-strip')).toHaveTextContent('Ana is typing...');
    typing('Bea');
    expect(screen.getByTestId('typing-strip')).toHaveTextContent('Ana and Bea are typing...');
    typing('Cid');
    expect(screen.getByTestId('typing-strip')).toHaveTextContent('Ana and 2 others are typing...');
    typing('Dan');
    typing('Eva');
    expect(screen.getByTestId('typing-strip')).toHaveTextContent('Ana and 4 others are typing...');
  });
});

describe('TypingIndicator in Japanese', () => {
  const hostile = '<img src=x onerror="alert(1)"> {{name}}';

  beforeEach(async () => {
    useUserCache.setState({
      users: new Map(
        ['h', 'Bea', 'Cid'].map((id) => [
          id,
          { id, email: `${id}@x.test`, display_name: id === 'h' ? hostile : id, avatar_url: null },
        ]),
      ),
    });
    await i18n.changeLanguage('ja');
  });

  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('says who is typing with さん, with names as text', () => {
    render(<TypingIndicator channelId="ch-1" currentUserId="me" />);
    typing('h');
    expect(screen.getByTestId('typing-strip').textContent).toBe(`${hostile} さんが入力中…`);
    typing('Bea');
    expect(screen.getByTestId('typing-strip').textContent).toBe(`${hostile} さんと Bea さんが入力中…`);
    typing('Cid');
    expect(screen.getByTestId('typing-strip').textContent).toBe(`${hostile} さんほか2人が入力中…`);
    expect(document.querySelector('[onerror]')).toBeNull();
  });
});
