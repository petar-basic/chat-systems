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

  private subscribedWorkspace: string | null = null;
  private joinedChannels = new Set<string>();
  private hasConnectedOnce = false;

  onReconnect: (() => void) | null = null;

  private static readonly RECONNECT_BASE_MS = 1000;
  private static readonly RECONNECT_FACTOR = 2;
  private static readonly RECONNECT_CAP_MS = 30000;

  onStatusChange: ((status: WsConnectionStatus) => void) | null = null;

  constructor(instanceUrl?: string, wsUrl?: string) {
    this.instanceUrl = instanceUrl;
    this.wsUrl = wsUrl;
  }

  connect() {
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
    this.doConnect();
  }

  private doConnect() {
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
    this.ws = new WebSocket(url);

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

      if (this.subscribedWorkspace) this.send({ type: 'subscribe', workspace_id: this.subscribedWorkspace });
      this.joinedChannels.forEach((channelId) => this.send({ type: 'channel.join', channel_id: channelId }));

      if (isReconnect) this.onReconnect?.();
    };

    this.ws.onmessage = (evt) => {
      try {
        const event = JSON.parse(evt.data) as ServerEvent;
        this.dispatch(event);
      } catch (e) {
        logger.error('WebSocketClient', 'onmessage', e);
      }
    };

    this.ws.onclose = () => {
      this.onStatusChange?.('disconnected');
      this.scheduleReconnect();
    };

    this.ws.onerror = (err) => {
      logger.error('WebSocketClient', 'onerror', err);
    };
  }

  disconnect() {
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
      this.doConnect();
    }, delay);
  }

  send(event: Record<string, unknown>) {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(event));
    }
  }

  subscribe(workspace_id: string) {
    this.subscribedWorkspace = workspace_id;
    this.send({ type: 'subscribe', workspace_id });
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

export const wsClient = new WebSocketClient();
