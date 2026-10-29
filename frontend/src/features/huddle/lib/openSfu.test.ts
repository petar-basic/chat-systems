import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { ActiveHuddle } from '@/stores/huddle';
import { openSfu } from './openSfu';

const post = vi.fn();
const join = vi.fn();
const closeSfu = vi.fn();
const sfuBuilt = vi.fn();

vi.mock('@/shared/hooks/useCurrentApi', () => ({
  getApiForInstance: () => ({
    typed: async (call: (c: { POST: typeof post }) => Promise<{ data: unknown }>) =>
      (await call({ POST: post })).data,
  }),
}));

vi.mock('./SfuTransport', () => ({
  SfuTransport: class {
    readonly kind = 'sfu';
    constructor(...args: unknown[]) {
      sfuBuilt(...args);
    }
    join = join;
    close = closeSfu;
  },
}));

const active: ActiveHuddle = {
  huddleId: 'h-1',
  workspaceId: 'ws-1',
  instanceUrl: 'https://chat.example.com',
  selfUserId: 'u-1',
  scope: { kind: 'channel', channelId: 'c-1' },
};
const ice: RTCIceServer[] = [{ urls: ['turn:turn.example.com'] }];
const handlers = { onTrack: vi.fn(), onSpeakers: vi.fn() };
const room = { url: 'wss://sfu.example.com', room: 'huddle-h-1', token: 't', expires_at: '' };

describe('openSfu', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    post.mockResolvedValue({ data: room });
    join.mockResolvedValue(undefined);
  });

  it('joins the huddle room on the SFU through the same relays', async () => {
    const transport = await openSfu(active, ice, handlers);

    expect(transport.kind).toBe('sfu');
    expect(post).toHaveBeenCalledWith('/huddles/{huddle_id}/sfu-token', {
      params: { path: { huddle_id: 'h-1' } },
    });
    expect(sfuBuilt).toHaveBeenCalledWith(room, ice, handlers);
    expect(join).toHaveBeenCalled();
  });

  it('closes what it opened when the SFU cannot be reached', async () => {
    join.mockRejectedValue(new Error('could not establish signal connection'));

    await expect(openSfu(active, ice, handlers)).rejects.toThrow('could not establish signal connection');
    expect(closeSfu).toHaveBeenCalled();
  });

  it('builds nothing when the instance has no SFU', async () => {
    post.mockRejectedValue(new Error('This instance has no SFU'));

    await expect(openSfu(active, ice, handlers)).rejects.toThrow('This instance has no SFU');
    expect(sfuBuilt).not.toHaveBeenCalled();
  });
});
