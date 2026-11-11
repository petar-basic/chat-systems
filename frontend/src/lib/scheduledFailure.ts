const FAILURES = [
  'not_authorized',
  'author_inactive',
  'channel_archived',
  'recipient_left',
  'workspace_unavailable',
  'thread_gone',
] as const;

export const failureKey = (failure: string | null | undefined) =>
  FAILURES.find((known) => known === failure) ?? 'unknown';
