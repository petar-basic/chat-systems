import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { ActiveHuddle } from '@/stores/huddle';
import { openTransport, SFU_FLAG } from './openTransport';

const post = vi.fn();
const join = vi.fn();
const closeSfu = vi.fn();
const sfuBuilt = vi.fn();
const meshBuilt = vi.fn();

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

vi.mock('./MeshManager', () => ({
  MeshManager: class {
    readonly kind = 'mesh';
    constructor(...args: unknown[]) {
      meshBuilt(...args);
    }
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
const send = vi.fn();
const room = { url: 'wss://sfu.example.com', room: 'huddle-h-1', token: 't', expires_at: '' };

describe('openTransport', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
    post.mockResolvedValue({ data: room });
    join.mockResolvedValue(undefined);
  });

  it('stays on the mesh and asks for no token without the flag', async () => {
    const transport = await openTransport(active, ice, send, handlers);

    expect(transport.kind).toBe('mesh');
    expect(meshBuilt).toHaveBeenCalledWith('h-1', 'u-1', ice, send, handlers);
    expect(post).not.toHaveBeenCalled();
  });

  it('joins the huddle room on the SFU with the flag, through the same relays', async () => {
    localStorage.setItem(SFU_FLAG, '1');

    const transport = await openTransport(active, ice, send, handlers);

    expect(transport.kind).toBe('sfu');
    expect(post).toHaveBeenCalledWith('/huddles/{huddle_id}/sfu-token', {
      params: { path: { huddle_id: 'h-1' } },
    });
    expect(sfuBuilt).toHaveBeenCalledWith(room, ice, handlers);
    expect(join).toHaveBeenCalled();
    expect(meshBuilt).not.toHaveBeenCalled();
  });

  it('falls back to the mesh when the SFU cannot be reached', async () => {
    localStorage.setItem(SFU_FLAG, '1');
    join.mockRejectedValue(new Error('could not establish signal connection'));

    const transport = await openTransport(active, ice, send, handlers);

    expect(closeSfu).toHaveBeenCalled();
    expect(transport.kind).toBe('mesh');
  });

  it('falls back to the mesh when the instance has no SFU', async () => {
    localStorage.setItem(SFU_FLAG, '1');
    post.mockRejectedValue(new Error('This instance has no SFU'));

    const transport = await openTransport(active, ice, send, handlers);

    expect(sfuBuilt).not.toHaveBeenCalled();
    expect(transport.kind).toBe('mesh');
  });
});
