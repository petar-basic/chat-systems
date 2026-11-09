import { describe, it, expect, beforeEach } from 'vitest';
import { act, render, screen } from '@testing-library/react';
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
