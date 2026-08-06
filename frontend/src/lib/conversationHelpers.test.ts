import { describe, it, expect } from 'vitest';
import { conversationTitle, otherParticipants, isUnread } from './conversationHelpers';
import type { Conversation } from '@/hooks/queries/useConversations';

const names: Record<string, string> = {
  me: 'Me',
  a: 'Alice Johnson',
  b: 'Bob Smith',
  c: 'Charlie Brown',
  d: 'Diana Prince',
  e: 'Eve Adams',
};
const nameOf = (id: string) => names[id] ?? null;

function conversation(over: Partial<Conversation>): Conversation {
  return {
    id: 'conv-1',
    workspace_id: 'ws-1',
    kind: 'direct',
    last_message_at: '2026-01-02T10:00:00Z',
    last_read_at: null,
    participant_ids: ['me', 'a'],
    ...over,
  };
}

describe('conversationHelpers', () => {
  it('names a direct conversation after the other person', () => {
    expect(conversationTitle(conversation({}), 'me', nameOf)).toBe('Alice Johnson');
  });

  it('lists a group by its members, without the viewer', () => {
    const group = conversation({ kind: 'group', participant_ids: ['me', 'a', 'b'] });

    expect(otherParticipants(group, 'me')).toEqual(['a', 'b']);
    expect(conversationTitle(group, 'me', nameOf)).toBe('Alice Johnson, Bob Smith');
  });

  it('summarises a crowded group instead of running off the sidebar', () => {
    const crowd = conversation({ kind: 'group', participant_ids: ['me', 'a', 'b', 'c', 'd', 'e'] });

    expect(conversationTitle(crowd, 'me', nameOf)).toBe('Alice Johnson, Bob Smith, Charlie Brown +2');
  });

  it('falls back to your own name in a conversation with only you left', () => {
    const alone = conversation({ participant_ids: ['me'] });

    expect(conversationTitle(alone, 'me', nameOf)).toBe('Me');
  });

  it('marks a conversation unread until the read marker catches up', () => {
    expect(isUnread(conversation({ last_read_at: null }))).toBe(true);
    expect(isUnread(conversation({ last_read_at: '2026-01-02T09:00:00Z' }))).toBe(true);
    expect(isUnread(conversation({ last_read_at: '2026-01-02T10:00:00Z' }))).toBe(false);
  });
});
