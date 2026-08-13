import { globalEventBus, type ServerEvent } from './globalEventBus';
import { logger } from './logger';

export type { ServerEvent };
export type WsConnectionStatus = 'connecting' | 'connected' | 'disconnected';

type EventHandler = (event: ServerEvent) => void;

export class WebSocketClient {
  private ws: WebSocket | null = null;
  private handlers: Map<string, Set<EventHandler>> = new Map();
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private reconnectAttempts = 0;
  private instanceUrl?: string;
  private wsUrl?: string;
  private getToken?: () => Promise<string | null>;
  private connectSeq = 0;

  private subscribedWorkspace: string | null = null;
  /**
   * The last position this client has processed, per workspace. Kept in memory
   * on purpose: a page load refetches everything anyway, so persisting it would
   * only add a stale-position failure mode for nothing.
   */
  private lastEventId = new Map<string, string>();
  private joinedChannels = new Set<string>();
  private hasConnectedOnce = false;

  private reconnectListeners = new Set<() => void>();

  addReconnectListener(listener: () => void): () => void {
    this.reconnectListeners.add(listener);
    return () => this.reconnectListeners.delete(listener);
  }

  private static readonly SESSION_REVOKED_CLOSE_CODE = 4001;
  /** The gateway dropped us for being too far behind; reconnect straight away. */
  private static readonly BACKPRESSURE_CLOSE_CODE = 4003;
  private static readonly RECONNECT_BASE_MS = 1000;
  private static readonly RECONNECT_FACTOR = 2;
  private static readonly RECONNECT_CAP_MS = 30000;

  onStatusChange: ((status: WsConnectionStatus) => void) | null = null;
  onSessionRevoked: ((reason: string) => void) | null = null;

  constructor(instanceUrl?: string, wsUrl?: string, getToken?: () => Promise<string | null>) {
    this.instanceUrl = instanceUrl;
    this.wsUrl = wsUrl;
    this.getToken = getToken;
  }

  connect() {
    this.connectSeq += 1;
    if (this.ws) {
      this.ws.onclose = null;
      this.ws.close();
      this.ws = null;
    }
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    this.reconnectAttempts = 0;
    this.hasConnectedOnce = false;
    void this.doConnect();
  }

  private async doConnect() {
    const seq = this.connectSeq;
    let url: string;
    if (this.wsUrl) {
      const base = this.wsUrl.replace(/\/$/, '');
      url = base.endsWith('/ws') ? base : `${base}/ws`;
    } else if (this.instanceUrl && this.instanceUrl !== window.location.origin) {
      url = this.instanceUrl.replace(/^http/, 'ws') + '/ws';
    } else {
      const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
      url = `${protocol}//${window.location.host}/ws`;
    }

    this.onStatusChange?.('connecting');
    let token: string | null = null;
    try {
      token = (await this.getToken?.()) ?? null;
    } catch {
      token = null;
    }
    if (seq !== this.connectSeq) return;
    this.ws = token ? new WebSocket(url, ['bearer', token]) : new WebSocket(url);

    this.ws.onopen = () => {
      const isReconnect = this.hasConnectedOnce;
      this.hasConnectedOnce = true;
      this.reconnectAttempts = 0;
      this.onStatusChange?.('connected');
      logger.info(
        'WebSocketClient',
        'onopen',
        `${isReconnect ? 're' : ''}connected${this.instanceUrl ? ` (${this.instanceUrl})` : ''}`,
      );

      // Channels first, then the workspace. Subscribing is what triggers the
      // replay, and a replayed event is routed through the same visibility
      // predicate as a live one — so a connection that has not re-declared its
      // channels yet would be handed nothing at all.
      this.joinedChannels.forEach((channelId) => this.send({ type: 'channel.join', channel_id: channelId }));
      if (this.subscribedWorkspace) {
        // Resuming from the last processed position is what turns a dropped
        // socket into a gap replay instead of a refetch of whatever is open.
        this.send({
          type: 'subscribe',
          workspace_id: this.subscribedWorkspace,
          last_event_id: this.lastEventId.get(this.subscribedWorkspace),
        });
      }

      if (isReconnect) this.reconnectListeners.forEach((listener) => listener());
    };

    this.ws.onmessage = (evt) => {
      try {
        const event = JSON.parse(evt.data) as ServerEvent;
        this.dispatch(event);
      } catch (e) {
        logger.error('WebSocketClient', 'onmessage', e);
      }
    };

    this.ws.onclose = (evt) => {
      this.onStatusChange?.('disconnected');
      // The gateway closes with SESSION_REVOKED when the session is no longer
      // valid. Reconnecting would loop against a handshake that can only fail.
      if (evt.code === WebSocketClient.SESSION_REVOKED_CLOSE_CODE) {
        logger.warn('WebSocketClient', 'session revoked', evt.reason);
        this.onSessionRevoked?.(evt.reason);
        return;
      }
      if (evt.code === WebSocketClient.BACKPRESSURE_CLOSE_CODE) {
        // Being told is the point: the old behaviour dropped the socket in
        // silence and waited for a heartbeat to notice. Reconnect now and
        // replay from the last processed position.
        logger.warn('WebSocketClient', 'dropped for backpressure', evt.reason);
        this.reconnectAttempts = 0;
      }
      this.scheduleReconnect();
    };

    this.ws.onerror = (err) => {
      logger.error('WebSocketClient', 'onerror', err);
    };
  }

  disconnect() {
    this.connectSeq += 1;
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    this.reconnectAttempts = 0;
    if (this.ws) {
      this.ws.onclose = null;
      this.ws.close();
      this.ws = null;
    }
    this.onStatusChange?.('disconnected');
  }

  private scheduleReconnect() {
    if (this.reconnectTimer) return;

    const exp =
      WebSocketClient.RECONNECT_BASE_MS * Math.pow(WebSocketClient.RECONNECT_FACTOR, this.reconnectAttempts);
    const capped = Math.min(exp, WebSocketClient.RECONNECT_CAP_MS);
    const min = WebSocketClient.RECONNECT_BASE_MS;
    const delay = Math.round(min + Math.random() * Math.max(capped - min, 0));
    this.reconnectAttempts += 1;

    logger.info('WebSocketClient', 'scheduleReconnect', `disconnected, retrying in ${delay}ms`);
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      void this.doConnect();
    }, delay);
  }

  send(event: Record<string, unknown>) {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(event));
    }
  }

  subscribe(workspace_id: string) {
    this.subscribedWorkspace = workspace_id;
    this.send({ type: 'subscribe', workspace_id, last_event_id: this.lastEventId.get(workspace_id) });
  }

  joinChannel(channel_id: string) {
    this.joinedChannels.add(channel_id);
    this.send({ type: 'channel.join', channel_id });
  }

  leaveChannel(channel_id: string) {
    this.joinedChannels.delete(channel_id);
    this.send({ type: 'channel.leave', channel_id });
  }

  on(type: string, handler: EventHandler) {
    if (!this.handlers.has(type)) {
      this.handlers.set(type, new Set());
    }
    this.handlers.get(type)!.add(handler);
    return () => this.handlers.get(type)?.delete(handler);
  }

  private dispatch(event: ServerEvent) {
    // The gateway hands out a starting position when the client has none, so a
    // socket that drops before it ever saw an event still has somewhere to
    // resume from.
    if (event.type === 'sync.complete' && event.last_event_id) {
      this.lastEventId.set(event.workspace_id, event.last_event_id);
    }

    // Replay overlaps the live tail on purpose, so the same event can arrive
    // twice. Most handlers upsert by id and would not care, but two of them
    // increment — a thread's reply count and the unread badge — and those would
    // double. Anything at or behind the position already processed is dropped
    // here, once, rather than every handler having to defend itself.
    const streamId = (event as { stream_id?: string }).stream_id;
    if (streamId && this.subscribedWorkspace) {
      const seen = this.lastEventId.get(this.subscribedWorkspace);
      if (seen && !isNewerStreamId(streamId, seen)) return;
      this.lastEventId.set(this.subscribedWorkspace, streamId);
    }

    const handlers = this.handlers.get(event.type);
    if (handlers) {
      handlers.forEach((h) => h(event));
    }
    const all = this.handlers.get('*');
    if (all) {
      all.forEach((h) => h(event));
    }
    globalEventBus.emit(event);
  }
}

/** Redis stream ids are `<millis>-<seq>`; both halves compare as numbers. */
export function isNewerStreamId(candidate: string, current: string): boolean {
  const parse = (id: string): [number, number] | null => {
    const [ms, seq] = id.split('-');
    const a = Number(ms);
    const b = Number(seq);
    return Number.isFinite(a) && Number.isFinite(b) ? [a, b] : null;
  };
  const next = parse(candidate);
  const seen = parse(current);
  if (!next || !seen) return true;
  return next[0] !== seen[0] ? next[0] > seen[0] : next[1] > seen[1];
}

export const wsClient = new WebSocketClient();
