import type { Message } from '@/stores/workspace';

export function isClosedThread(root: Message, loaded: Message[], loading: boolean): boolean {
  return !!root.deleted_at && !loading && loaded.every((reply) => reply.failed);
}
