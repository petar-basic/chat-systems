import { describe, it, expect, vi, beforeEach, afterEach, type Mock } from 'vitest';
import type { HuddleTransport, TransportHandlers, TransportKind } from './HuddleTransport';
import { HANDOVER_TIMEOUT_MS, Promotion } from './Promotion';

const fakeTransport = (kind: TransportKind) => ({
  kind,
  setTrack: vi.fn(),
  setMuted: vi.fn(),
  addPeer: vi.fn(),
  removePeer: vi.fn(),
  restart: vi.fn(),
  close: vi.fn(),
});

const stream = (name: string) => ({ id: name }) as unknown as MediaStream;
const track = (name: string) => ({ id: name }) as unknown as MediaStreamTrack;

const settle = () => new Promise<void>((resolve) => queueMicrotask(resolve));

let mesh: ReturnType<typeof fakeTransport>;
let sfu: ReturnType<typeof fakeTransport>;
let handlers: {
  onTrack: Mock<TransportHandlers['onTrack']>;
  onSpeakers: Mock<TransportHandlers['onSpeakers']>;
};
let sfuHandlers: TransportHandlers;
const mic = track('mic');

const promotion = (peers: string[]) =>
  new Promotion(mesh, peers, handlers, false, { audio: mic, camera: null, screen: null });

const open = (handed: TransportHandlers): Promise<HuddleTransport> => {
  sfuHandlers = handed;
  return Promise.resolve(sfu);
};

describe('Promotion', () => {
  beforeEach(() => {
    mesh = fakeTransport('mesh');
    sfu = fakeTransport('sfu');
    handlers = {
      onTrack: vi.fn<TransportHandlers['onTrack']>(),
      onSpeakers: vi.fn<TransportHandlers['onSpeakers']>(),
    };
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('keeps the mesh playing until every peer is heard on the SFU, then hands over at once', async () => {
    const p = promotion(['b', 'c']);
    const done = p.run(open);
    await settle();

    sfuHandlers.onTrack('b', stream('b-audio'), 'audio');
    sfuHandlers.onTrack('c', stream('c-camera'), 'camera');
    sfuHandlers.onSpeakers(['b']);
    await settle();

    expect(handlers.onTrack).not.toHaveBeenCalled();
    expect(handlers.onSpeakers).not.toHaveBeenCalled();
    expect(mesh.close).not.toHaveBeenCalled();
    expect(p.kind).toBe('mesh');

    sfuHandlers.onTrack('c', stream('c-audio'), 'audio');
    await done;

    expect(mesh.close).toHaveBeenCalled();
    expect(p.kind).toBe('sfu');
    expect(handlers.onTrack.mock.calls).toEqual([
      ['b', stream('b-audio'), 'audio'],
      ['c', stream('c-camera'), 'camera'],
      ['c', stream('c-audio'), 'audio'],
    ]);

    sfuHandlers.onTrack('b', null, 'camera');
    sfuHandlers.onSpeakers(['c']);
    expect(handlers.onTrack).toHaveBeenLastCalledWith('b', null, 'camera');
    expect(handlers.onSpeakers).toHaveBeenCalledWith(['c']);
  });

  it('publishes the local microphone and mute on the SFU, and sends every change to both while it switches', async () => {
    const p = new Promotion(mesh, ['b'], handlers, true, { audio: mic, camera: null, screen: null });
    const done = p.run(open);
    await settle();

    expect(sfu.setMuted).toHaveBeenCalledWith(true);
    expect(sfu.setTrack).toHaveBeenCalledWith('audio', mic);
    expect(sfu.setTrack).toHaveBeenCalledWith('camera', null);
    expect(sfu.setTrack).toHaveBeenCalledWith('screen', null);

    const camera = track('camera');
    p.setMuted(false);
    p.setTrack('camera', camera);
    expect(mesh.setMuted).toHaveBeenCalledWith(false);
    expect(sfu.setMuted).toHaveBeenLastCalledWith(false);
    expect(mesh.setTrack).toHaveBeenCalledWith('camera', camera);
    expect(sfu.setTrack).toHaveBeenLastCalledWith('camera', camera);

    sfuHandlers.onTrack('b', stream('b-audio'), 'audio');
    await done;
    mesh.setMuted.mockClear();

    p.setMuted(true);
    expect(mesh.setMuted).not.toHaveBeenCalled();
    expect(sfu.setMuted).toHaveBeenLastCalledWith(true);
  });

  it('does not wait for someone who leaves during the switch', async () => {
    const p = promotion(['b', 'c']);
    const done = p.run(open);
    await settle();

    sfuHandlers.onTrack('b', stream('b-audio'), 'audio');
    p.removePeer('c');
    await done;

    expect(mesh.removePeer).toHaveBeenCalledWith('c');
    expect(p.kind).toBe('sfu');
  });

  it('hands over after a while even when somebody never reaches the SFU', async () => {
    vi.useFakeTimers();
    const p = promotion(['b']);
    const done = p.run(open);
    await vi.advanceTimersByTimeAsync(HANDOVER_TIMEOUT_MS - 1);
    expect(mesh.close).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(1);
    await done;

    expect(mesh.close).toHaveBeenCalled();
    expect(p.kind).toBe('sfu');
  });

  it('switches straight away when there is nobody on the mesh to wait for', async () => {
    const p = promotion([]);
    await p.run(open);

    expect(mesh.close).toHaveBeenCalled();
    expect(p.kind).toBe('sfu');
  });

  it('never meshes with anyone new once the huddle has moved', async () => {
    const p = promotion(['b']);
    void p.run(open);
    await settle();

    p.addPeer('d');
    p.close();

    expect(mesh.addPeer).not.toHaveBeenCalled();
  });

  it('leaves the mesh alone when the SFU cannot be reached', async () => {
    const p = promotion(['b']);

    await expect(p.run(() => Promise.reject(new Error('This instance has no SFU')))).rejects.toThrow();

    expect(mesh.close).not.toHaveBeenCalled();
    expect(handlers.onTrack).not.toHaveBeenCalled();
  });

  it('closes both and hands nothing over when the call ends mid-switch', async () => {
    const p = promotion(['b']);
    const done = p.run(open);
    await settle();
    sfuHandlers.onTrack('c', stream('c-audio'), 'audio');

    p.close();
    await done;

    expect(mesh.close).toHaveBeenCalled();
    expect(sfu.close).toHaveBeenCalled();
    expect(handlers.onTrack).not.toHaveBeenCalled();
  });

  it('closes an SFU that finishes joining after the call ended', async () => {
    let finish: (t: HuddleTransport) => void = () => {};
    const p = promotion(['b']);
    const done = p.run(() => new Promise<HuddleTransport>((resolve) => (finish = resolve)));

    p.close();
    finish(sfu);
    await done;

    expect(sfu.close).toHaveBeenCalled();
    expect(sfu.setTrack).not.toHaveBeenCalled();
  });
});
