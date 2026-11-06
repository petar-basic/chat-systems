import { describe, it, expect } from 'vitest';
import { conversationTitle, departedPartner, departureNote, otherParticipants } from './conversationHelpers';
import type { Conversation } from '@/features/messaging/hooks/useConversations';

const names: Record<string, string> = {
  me: 'Me',
  a: 'Alice Johnson',
  b: 'Bob Smith',
  c: 'Charlie Brown',
  d: 'Diana Prince',
  e: 'Eve Adams',
};
const nameOf = (id: string) => names[id] ?? null;

function gone(userId: string, displayName: string) {
  return { user_id: userId, display_name: displayName, avatar_url: null, left_at: '2026-01-01T10:00:00Z' };
}

function conversation(over: Partial<Conversation>): Conversation {
  return {
    id: 'conv-1',
    workspace_id: 'ws-1',
    kind: 'direct',
    last_message_at: '2026-01-02T10:00:00Z',
    unread_count: 0,
    participant_ids: ['me', 'a'],
    notify_level: null,
    effective_notify_level: 'all',
    departed: [],
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

  it('keeps the name of somebody who left the workspace', () => {
    const left = conversation({ participant_ids: ['me'], departed: [gone('b', 'Bob Smith')] });

    expect(conversationTitle(left, 'me', nameOf)).toBe('Bob Smith');
    expect(departedPartner(left, 'me')?.user_id).toBe('b');
    expect(departureNote(left)).toBe('Bob Smith is no longer in this workspace');
  });

  it('names who left a group beside who is still in it', () => {
    const group = conversation({
      kind: 'group',
      participant_ids: ['me', 'a'],
      departed: [gone('b', 'Bob Smith'), gone('c', 'Charlie Brown')],
    });

    expect(conversationTitle(group, 'me', nameOf)).toBe('Alice Johnson, Bob Smith, Charlie Brown');
    expect(departedPartner(group, 'me')).toBeNull();
    expect(departureNote(group)).toBe('Bob Smith and Charlie Brown are no longer in this workspace');
  });

  it('has nothing to say about a conversation everybody is still in', () => {
    expect(departedPartner(conversation({}), 'me')).toBeNull();
    expect(departureNote(conversation({}))).toBeNull();
  });
});
