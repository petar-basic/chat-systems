import { describe, it, expect } from 'vitest';
import { QueryClient } from '@tanstack/react-query';
import type { Message } from '@/stores/workspace';
import { QUERY_KEYS } from '@/shared/constants';
import { dropReply, oldestFirst, patchReply, placeReply, type ThreadReplies } from './threadCache';

const reply = (id: string, at: string, parent = 'root'): Message => ({
  id,
  channel_id: 'ch',
  user_id: 'u',
  client_message_id: null,
  content: id,
  thread_parent_id: parent,
  reply_count: 0,
  is_pinned: false,
  created_at: at,
  updated_at: at,
  deleted_at: null,
});

const seeded = () => {
  const queryClient = new QueryClient();
  const pages: ThreadReplies = {
    pages: [
      { data: [reply('r4', '2026-10-01T04:00:00Z'), reply('r3', '2026-10-01T03:00:00Z')] },
      { data: [reply('r2', '2026-10-01T02:00:00Z'), reply('r1', '2026-10-01T01:00:00Z')] },
    ],
    pageParams: [null, 'r3'],
  };
  queryClient.setQueryData(QUERY_KEYS.thread('root'), pages);
  const ids = () =>
    oldestFirst(queryClient.getQueryData<ThreadReplies>(QUERY_KEYS.thread('root'))).map((m) => m.id);
  return { queryClient, ids };
};

describe('threadCache', () => {
  it('reads every loaded page oldest first', () => {
    expect(seeded().ids()).toEqual(['r1', 'r2', 'r3', 'r4']);
  });

  it('places a new reply last and replaces its optimistic copy', () => {
    const { queryClient, ids } = seeded();
    placeReply(queryClient, { ...reply('tmp', '2026-10-01T05:00:00Z'), pending: true });
    expect(ids()).toEqual(['r1', 'r2', 'r3', 'r4', 'tmp']);

    placeReply(queryClient, reply('r5', '2026-10-01T05:00:01Z'), 'tmp');
    expect(ids()).toEqual(['r1', 'r2', 'r3', 'r4', 'r5']);
  });

  it('patches and drops a reply in whichever loaded page holds it', () => {
    const { queryClient, ids } = seeded();
    patchReply(queryClient, 'r1', (m) => ({ ...m, content: 'edited' }));
    const thread = oldestFirst(queryClient.getQueryData<ThreadReplies>(QUERY_KEYS.thread('root')));
    expect(thread[0].content).toBe('edited');

    dropReply(queryClient, 'r2');
    expect(ids()).toEqual(['r1', 'r3', 'r4']);
  });

  it('leaves a thread that is not open alone', () => {
    const queryClient = new QueryClient();
    placeReply(queryClient, reply('r1', '2026-10-01T01:00:00Z', 'elsewhere'));
    expect(queryClient.getQueryData(QUERY_KEYS.thread('elsewhere'))).toBeUndefined();
  });
});
