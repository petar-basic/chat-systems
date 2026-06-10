import { ApiClient } from './api';
import { WebSocketClient } from './ws';

export interface InstanceClients {
  api: ApiClient;
  ws: WebSocketClient;
}

class InstanceManager {
  private clients = new Map<string, InstanceClients>();
  private wsUrls = new Map<string, string>();

  normalize(url: string): string {
    return url.replace(/\/$/, '');
  }

  setWsUrl(instanceUrl: string, wsUrl: string) {
    this.wsUrls.set(this.normalize(instanceUrl), wsUrl);
  }

  get(instanceUrl: string): InstanceClients {
    const key = this.normalize(instanceUrl);
    if (!this.clients.has(key)) {
      const wsUrl = this.wsUrls.get(key);
      const api = new ApiClient(key);
      const ws = new WebSocketClient(key, wsUrl, () => api.getValidToken());
      this.clients.set(key, { api, ws });
    }
    return this.clients.get(key)!;
  }

  remove(instanceUrl: string) {
    const key = this.normalize(instanceUrl);
    const clients = this.clients.get(key);
    if (clients) {
      clients.ws.disconnect();
      this.clients.delete(key);
    }
    this.wsUrls.delete(key);
  }

  all(): Map<string, InstanceClients> {
    return this.clients;
  }
}

export const instanceManager = new InstanceManager();
