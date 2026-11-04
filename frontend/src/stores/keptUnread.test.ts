import { describe, it, expect, beforeEach } from 'vitest';
import { useWorkspaceStore } from './workspace';
import { isKeptUnread, keepUnread, releaseUnread, useKeptUnread } from './keptUnread';

describe('a channel marked unread', () => {
  beforeEach(() => {
    useKeptUnread.setState({ channelId: null });
    useWorkspaceStore.setState({ currentChannel: { id: 'ch-1' } as never, currentConversationId: null });
  });

  it('is kept unread while it is the one open', () => {
    keepUnread('ch-1');
    expect(isKeptUnread('ch-1')).toBe(true);
  });

  it('is not held when it was marked from somewhere else, so opening it still reads it', () => {
    keepUnread('ch-2');
    expect(isKeptUnread('ch-2')).toBe(false);
  });

  it('holds a conversation the same way', () => {
    useWorkspaceStore.setState({ currentChannel: null, currentConversationId: 'dm-1' });
    keepUnread('dm-1');
    expect(isKeptUnread('dm-1')).toBe(true);
  });

  it('lets go once the channel is left, and only for that channel', () => {
    keepUnread('ch-1');
    releaseUnread('ch-2');
    expect(isKeptUnread('ch-1')).toBe(true);
    releaseUnread('ch-1');
    expect(isKeptUnread('ch-1')).toBe(false);
  });
});
