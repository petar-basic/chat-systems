import { create } from 'zustand';
import { instanceManager } from '../lib/instances';
import { useWsStatusStore } from './wsStatus';
import { backfillAfterReconnect } from '../lib/realtimeBackfill';
import { toast } from '@/shared/components/Toast';
import { ErrorLabels } from '@/shared/constants';

export interface InstanceUser {
  id: string;
  email: string;
  display_name: string;
  avatar_url: string | null;
  is_instance_admin: boolean;
}

export interface InstanceConfig {
  url: string;
  wsUrl?: string;
  user: InstanceUser;
}

const STORAGE_KEY = 'chat_instances';

let restorePromise: Promise<void> | null = null;

function loadFromStorage(): InstanceConfig[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as Array<Record<string, unknown>>;
    return parsed.map(({ url, wsUrl, user }) => ({
      url: url as string,
      ...(wsUrl ? { wsUrl: wsUrl as string } : {}),
      user: user as InstanceUser,
    }));
  } catch {
    return [];
  }
}

function saveToStorage(instances: InstanceConfig[]) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(instances));
}

interface InstancesState {
  instances: InstanceConfig[];
  activeInstanceUrl: string | null;
  hydrated: boolean;
  loading: boolean;
  error: string | null;

  restoreInstances: () => Promise<void>;
  addInstance: (url: string, email: string, password: string, wsUrl?: string) => Promise<void>;
  addValidatedInstance: (config: InstanceConfig) => void;
  removeInstance: (url: string) => void;
  setActiveInstance: (url: string) => void;
  updateInstanceUser: (url: string, user: InstanceUser) => void;
  clearError: () => void;
}

export const useInstanceStore = create<InstancesState>((set, get) => ({
  instances: [],
  activeInstanceUrl: null,
  hydrated: false,
  loading: false,
  error: null,

  restoreInstances: async () => {
    if (restorePromise) return restorePromise;
    restorePromise = (async () => {
      const saved = loadFromStorage();
      const valid: InstanceConfig[] = [];

      for (const config of saved) {
        if (config.wsUrl) {
          instanceManager.setWsUrl(config.url, config.wsUrl);
        }
        const clients = instanceManager.get(config.url);
        clients.api.onSessionExpired = () => {
          toast.error(ErrorLabels.SessionExpired);
          get().removeInstance(config.url);
        };
        clients.ws.onStatusChange = (status) => {
          useWsStatusStore.getState().setStatus(config.url, status);
        };
        clients.ws.onReconnect = backfillAfterReconnect;

        try {
          const normalized = instanceManager.normalize(config.url);
          if (normalized !== window.location.origin) {
            const refreshed = await clients.api
              .post<{ user: InstanceUser; access_token: string }>('/auth/refresh', {})
              .catch(() => null);
            if (!refreshed) {
              instanceManager.remove(config.url);
              continue;
            }
            clients.api.setToken(refreshed.access_token);
            valid.push({
              url: config.url,
              ...(config.wsUrl ? { wsUrl: config.wsUrl } : {}),
              user: refreshed.user,
            });
          } else {
            const user = await clients.api.get<InstanceUser>('/users/me');
            valid.push({ url: config.url, ...(config.wsUrl ? { wsUrl: config.wsUrl } : {}), user });
          }
          clients.ws.connect();
        } catch {
          instanceManager.remove(config.url);
        }
      }

      saveToStorage(valid);
      set({ instances: valid, activeInstanceUrl: valid[0]?.url ?? null, hydrated: true });
    })();
    return restorePromise;
  },

  addInstance: async (url, email, password, wsUrl?) => {
    set({ loading: true, error: null });
    const normalized = instanceManager.normalize(url);
    const normalizedWsUrl = wsUrl?.trim() || undefined;
    try {
      if (normalizedWsUrl) {
        instanceManager.setWsUrl(normalized, normalizedWsUrl);
      }
      const clients = instanceManager.get(normalized);
      clients.api.onSessionExpired = () => {
        toast.error(ErrorLabels.SessionExpired);
        get().removeInstance(normalized);
      };
      clients.ws.onStatusChange = (status) => {
        useWsStatusStore.getState().setStatus(normalized, status);
      };
      clients.ws.onReconnect = backfillAfterReconnect;

      const res = await clients.api.post<{ user: InstanceUser; expires_in: number; access_token: string }>(
        '/auth/login',
        { email, password },
      );

      clients.api.setToken(res.access_token);
      clients.ws.connect();

      const config: InstanceConfig = {
        url: normalized,
        ...(normalizedWsUrl ? { wsUrl: normalizedWsUrl } : {}),
        user: res.user,
      };

      const existing = get().instances.filter((i) => i.url !== normalized);
      const instances = [...existing, config];
      saveToStorage(instances);
      set({ instances, activeInstanceUrl: normalized, loading: false });
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : 'Failed to connect to instance';
      set({ loading: false, error: msg });
      throw e;
    }
  },

  addValidatedInstance: (config) => {
    const existing = get().instances.filter((i) => i.url !== config.url);
    const instances = [...existing, config];
    saveToStorage(instances);
    set({ instances, activeInstanceUrl: config.url });
  },

  removeInstance: (url) => {
    const normalized = instanceManager.normalize(url);
    instanceManager
      .get(normalized)
      .api.post('/auth/logout', {})
      .catch(() => {});
    instanceManager.remove(normalized);
    const instances = get().instances.filter((i) => i.url !== normalized);
    saveToStorage(instances);
    set({ instances, activeInstanceUrl: instances[0]?.url ?? null });
  },

  setActiveInstance: (url) => {
    set({ activeInstanceUrl: instanceManager.normalize(url) });
  },

  updateInstanceUser: (url, user) => {
    const normalized = instanceManager.normalize(url);
    const instances = get().instances.map((i) => (i.url === normalized ? { ...i, user } : i));
    saveToStorage(instances);
    set({ instances });
  },

  clearError: () => set({ error: null }),
}));
