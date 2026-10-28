import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

vi.mock('./activity', () => ({
  isActive: () => false,
  watchActivity: () => () => undefined,
}));

class FakeSocket {
  static CONNECTING = 0;
  static OPEN = 1;
  static CLOSED = 3;
  static all: FakeSocket[] = [];
  readyState = FakeSocket.CONNECTING;
  sent: unknown[] = [];
  onopen: (() => void) | null = null;
  onclose: ((evt: { code: number }) => void) | null = null;
  onmessage: ((evt: { data: string }) => void) | null = null;
  onerror: ((err: unknown) => void) | null = null;
  constructor() {
    FakeSocket.all.push(this);
  }
  send(data: string) {
    this.sent.push(JSON.parse(data));
  }
  close() {
    this.readyState = FakeSocket.CLOSED;
  }
  open() {
    this.readyState = FakeSocket.OPEN;
    this.onopen?.();
  }
  drop() {
    this.readyState = FakeSocket.CLOSED;
    this.onclose?.({ code: 1006 });
  }
  receive(frame: unknown) {
    this.onmessage?.({ data: JSON.stringify(frame) });
  }
}

let visibility: DocumentVisibilityState = 'visible';
const clients: Array<{ disconnect: () => void }> = [];

async function connectedClient() {
  const { WebSocketClient } = await import('./ws');
  const client = new WebSocketClient();
  clients.push(client);
  client.connect();
  await vi.waitFor(() => expect(FakeSocket.all).toHaveLength(1));
  FakeSocket.all[0].open();
  return client;
}

describe('WebSocketClient waking up', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    FakeSocket.all = [];
    visibility = 'visible';
    vi.stubGlobal('WebSocket', FakeSocket);
    vi.spyOn(document, 'visibilityState', 'get').mockImplementation(() => visibility);
    vi.spyOn(Math, 'random').mockReturnValue(1);
  });
  afterEach(() => {
    clients.splice(0).forEach((client) => client.disconnect());
    vi.useRealTimers();
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it('reconnects the moment the network comes back instead of waiting out the backoff', async () => {
    await connectedClient();
    FakeSocket.all[0].drop();
    expect(FakeSocket.all).toHaveLength(1);

    window.dispatchEvent(new Event('online'));
    await vi.waitFor(() => expect(FakeSocket.all).toHaveLength(2));

    await vi.advanceTimersByTimeAsync(60_000);
    expect(FakeSocket.all, 'the pending backoff retry was cancelled').toHaveLength(2);
  });

  it('reconnects when the tab comes back with a dead socket', async () => {
    await connectedClient();
    FakeSocket.all[0].drop();

    visibility = 'visible';
    document.dispatchEvent(new Event('visibilitychange'));
    await vi.waitFor(() => expect(FakeSocket.all).toHaveLength(2));
  });

  it('probes a socket that still looks open and replaces it when nothing answers', async () => {
    await connectedClient();
    const first = FakeSocket.all[0];

    document.dispatchEvent(new Event('visibilitychange'));
    expect(first.sent).toContainEqual({ type: 'ping' });
    await vi.advanceTimersByTimeAsync(2_000);

    await vi.waitFor(() => expect(FakeSocket.all).toHaveLength(2));
    expect(first.onclose, 'the old socket can no longer schedule a reconnect of its own').toBeNull();
  });

  it('keeps a socket that answers the probe', async () => {
    await connectedClient();
    const first = FakeSocket.all[0];

    document.dispatchEvent(new Event('visibilitychange'));
    first.receive({ type: 'pong' });
    await vi.advanceTimersByTimeAsync(2_000);

    expect(FakeSocket.all).toHaveLength(1);
  });

  it('asks again for a channel it was refused while only previewing it, once it is joined', async () => {
    const client = await connectedClient();
    const socket = FakeSocket.all[0];
    client.joinChannels(['c1']);
    client.joinChannels(['c1']);
    expect(socket.sent.filter((f) => JSON.stringify(f).includes('c1'))).toHaveLength(1);

    client.rejoinChannel('c1');

    expect(socket.sent.filter((f) => JSON.stringify(f).includes('c1'))).toHaveLength(2);
  });

  it('stays down after a deliberate disconnect', async () => {
    const client = await connectedClient();
    client.disconnect();

    window.dispatchEvent(new Event('online'));
    document.dispatchEvent(new Event('visibilitychange'));
    await vi.advanceTimersByTimeAsync(5_000);

    expect(FakeSocket.all).toHaveLength(1);
  });
});
