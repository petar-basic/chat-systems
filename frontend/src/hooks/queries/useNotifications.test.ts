import { describe, it, expect } from 'vitest';
import { normalizeNotification, type RawNotification } from './useNotifications';

const raw = (overrides: Partial<RawNotification> = {}): RawNotification => ({
  id: 'n1',
  workspace_id: 'ws1',
  user_id: 'u1',
  notification_type: 'mention',
  title: 'You were mentioned',
  body: 'hey @you',
  data: { channel_id: 'ch1', message_id: 'msg1' },
  is_read: false,
  created_at: '2026-01-01T00:00:00Z',
  ...overrides,
});

describe('normalizeNotification', () => {
  it('flattens channel_id/message_id out of the data blob (the deep-link fix)', () => {
    const n = normalizeNotification(raw());
    expect(n.channel_id).toBe('ch1');
    expect(n.message_id).toBe('msg1');
  });

  it('defaults missing data fields to null (not undefined) so navigation guards work', () => {
    const n = normalizeNotification(raw({ data: null }));
    expect(n.channel_id).toBeNull();
    expect(n.message_id).toBeNull();
  });

  it('coerces a null body to an empty string', () => {
    const n = normalizeNotification(raw({ body: null }));
    expect(n.body).toBe('');
  });
});
