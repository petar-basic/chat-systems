import { describe, it, expect } from 'vitest';
import type { Message } from '@/stores/workspace';
import { isClosedThread } from './closedThread';

const message = (fields: Partial<Message>) => ({ id: 'm', ...fields }) as Message;

describe('isClosedThread', () => {
  const deleted = message({ deleted_at: '2026-10-28T10:00:00Z' });

  it('closes a deleted root once its last reply is gone', () => {
    expect(isClosedThread(deleted, [], false)).toBe(true);
    expect(isClosedThread(deleted, [message({ failed: true })], false)).toBe(true);
  });

  it('keeps a thread open while it has a reply, its root is there, or it is still loading', () => {
    expect(isClosedThread(deleted, [message({ id: 'r-1' })], false)).toBe(false);
    expect(isClosedThread(message({}), [], false)).toBe(false);
    expect(isClosedThread(deleted, [], true)).toBe(false);
  });
});
