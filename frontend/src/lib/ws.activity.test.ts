import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

let active = false;
let report: ((active: boolean) => void) | undefined;

vi.mock('./activity', () => ({
  isActive: () => active,
  watchActivity: (listener: (active: boolean) => void) => {
    report = listener;
    return () => undefined;
  },
}));

class FakeSocket {
  static OPEN = 1;
  static last: FakeSocket | undefined;
  readyState = 0;
  sent: unknown[] = [];
  onopen: (() => void) | null = null;
  onclose: ((evt: { code: number }) => void) | null = null;
  onmessage: ((evt: { data: string }) => void) | null = null;
  onerror: ((err: unknown) => void) | null = null;
  constructor() {
    FakeSocket.last = this;
  }
  send(data: string) {
    this.sent.push(JSON.parse(data));
  }
  close() {}
  open() {
    this.readyState = FakeSocket.OPEN;
    this.onopen?.();
  }
}

async function connected(client: { connect: () => void }) {
  client.connect();
  await vi.waitFor(() => expect(FakeSocket.last).toBeDefined());
  FakeSocket.last!.open();
  return FakeSocket.last!;
}

describe('WebSocketClient activity', () => {
  beforeEach(() => {
    active = false;
    report = undefined;
    FakeSocket.last = undefined;
    vi.stubGlobal('WebSocket', FakeSocket);
  });
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('tells the gateway when the person starts and stops using the app', async () => {
    const { WebSocketClient } = await import('./ws');
    const socket = await connected(new WebSocketClient());
    expect(socket.sent).toEqual([]);

    report?.(true);
    report?.(false);
    expect(socket.sent).toEqual([
      { type: 'activity', active: true },
      { type: 'activity', active: false },
    ]);
  });

  it('says it is active again on a new socket, which starts out idle', async () => {
    active = true;
    const { WebSocketClient } = await import('./ws');
    const socket = await connected(new WebSocketClient());

    expect(socket.sent).toContainEqual({ type: 'activity', active: true });
  });
});
