import { describe, it, expect, beforeEach, vi } from 'vitest';
import { ApiError } from '@/lib/errors';
import { useDraftStore, type DraftPlace } from '@/stores/drafts';
import { useInstanceStore } from '@/stores/instances';
import { flushDraft, reconcileDrafts } from './draftSync';

const put = vi.fn();
const get = vi.fn();

vi.mock('@/shared/hooks/useCurrentApi', () => ({
  getApiForInstance: () => ({
    typed: async (call: (c: { GET: typeof get; PUT: typeof put }) => Promise<{ data: unknown }>) =>
      (await call({ GET: get, PUT: put })).data,
  }),
}));

const URL = 'https://chat.example';
const general: DraftPlace = {
  instanceUrl: URL,
  workspaceId: 'ws-1',
  channelId: 'ch-1',
  threadParentId: null,
};
const store = () => useDraftStore.getState();

const echo = (body: { content: string; updated_at: string; channel_id: string }) => ({
  data: {
    id: 'd-1',
    workspace_id: 'ws-1',
    channel_id: body.channel_id,
    thread_parent_id: null,
    content: body.content,
    files_left_behind: false,
    updated_at: body.updated_at,
  },
});

describe('draftSync', () => {
  beforeEach(() => {
    localStorage.clear();
    useDraftStore.setState({ drafts: {} });
    useInstanceStore.setState({
      instances: [
        {
          url: URL,
          user: {
            id: 'ana',
            email: 'ana@chat.example',
            display_name: 'Ana',
            avatar_url: null,
            instance_role: 'member',
          },
        },
      ],
      activeInstanceUrl: URL,
    });
    put.mockReset();
    get.mockReset();
  });

  it('sends a place up once at a time, then sends what was typed meanwhile', async () => {
    let release: () => void = () => {};
    put.mockImplementationOnce(
      (_path: string, { body }: { body: { content: string; updated_at: string; channel_id: string } }) =>
        new Promise((resolve) => {
          release = () => resolve(echo(body));
        }),
    );
    put.mockImplementation(async (_path: string, { body }: { body: never }) => echo(body));

    store().setDraft('ch-1', 'first', { place: general });
    const first = flushDraft('ch-1');
    store().setDraft('ch-1', 'first and more', { place: general });
    void flushDraft('ch-1');
    expect(put).toHaveBeenCalledTimes(1);

    release();
    await first;
    await vi.waitFor(() => expect(put).toHaveBeenCalledTimes(2));
    expect(put.mock.calls[1][1].body.content).toBe('first and more');
    await vi.waitFor(() => expect(store().entry('ch-1')?.pending).toBe(false));
    expect(store().getDraft('ch-1')).toBe('first and more');
  });

  it('keeps a refused draft here and stops sending it', async () => {
    put.mockRejectedValue(new ApiError(404, 'Not found'));
    store().setDraft('ch-1', 'for a room I left', { place: general });
    await flushDraft('ch-1');
    expect(store().entry('ch-1')).toEqual({ text: 'for a room I left', at: expect.any(String) });
  });

  it('keeps a draft pending when the network is down, to try again later', async () => {
    put.mockRejectedValue(new TypeError('Failed to fetch'));
    store().setDraft('ch-1', 'on a plane', { place: general });
    await flushDraft('ch-1');
    expect(store().entry('ch-1')?.pending).toBe(true);
  });

  it('on reconnect sends what waited, takes what the server has and drops what it no longer has', async () => {
    put.mockImplementation(async (_path: string, { body }: { body: never }) => echo(body));
    store().accept(
      URL,
      echo({ channel_id: 'ch-gone', content: 'sent elsewhere', updated_at: '2026-11-14T09:00:00Z' }).data,
    );
    store().setDraft('ch-1', 'typed offline', { place: general });
    get.mockResolvedValue({
      data: {
        data: [
          echo({ channel_id: 'ch-1', content: 'typed offline', updated_at: '2026-11-14T10:00:00Z' }).data,
          echo({ channel_id: 'ch-2', content: 'from the desktop', updated_at: '2026-11-14T10:00:00Z' }).data,
        ],
      },
    });

    await reconcileDrafts(URL, 'ws-1');

    expect(put).toHaveBeenCalledTimes(1);
    expect(store().entry('ch-1')).toMatchObject({ text: 'typed offline', pending: false });
    expect(store().getDraft('ch-2')).toBe('from the desktop');
    expect(store().getDraft('ch-gone')).toBe('');
  });
});
