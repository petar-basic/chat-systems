import { create } from 'zustand';
import { instanceManager } from '../lib/instances';
import { useWsStatusStore } from './wsStatus';
import { backfillAfterReconnect } from '../lib/realtimeBackfill';
import { toast } from '@/shared/components/Toast';
import { ErrorLabels } from '@/shared/constants';
import type { components } from '@/api/schema';

export interface InstanceUser {
  id: string;
  email: string;
  display_name: string;
  avatar_url: string | null;
  is_instance_admin: boolean;
}

export function toInstanceUser(user: components['schemas']['UserPublic']): InstanceUser {
  return {
    id: user.id,
    email: user.email,
    display_name: user.display_name ?? '',
    avatar_url: user.avatar_url ?? null,
    is_instance_admin: user.is_instance_admin,
  };
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

const TOKENS_KEY = 'chat_tokens';

interface StoredTokens {
  access: string;
  refresh: string;
}

function loadTokens(url: string): StoredTokens | null {
  try {
    const raw = localStorage.getItem(TOKENS_KEY);
    if (!raw) return null;
    const map = JSON.parse(raw) as Record<string, StoredTokens>;
    return map[url] ?? null;
  } catch {
    return null;
  }
}

function saveTokens(url: string, access: string | null, refresh: string | null) {
  try {
    const raw = localStorage.getItem(TOKENS_KEY);
    const map = raw ? (JSON.parse(raw) as Record<string, StoredTokens>) : {};
    if (access && refresh) map[url] = { access, refresh };
    else delete map[url];
    localStorage.setItem(TOKENS_KEY, JSON.stringify(map));
  } catch {
    return;
  }
}

interface InstancesState {
  instances: InstanceConfig[];
  activeInstanceUrl: string | null;
  hydrated: boolean;
  loading: boolean;
  error: string | null;

  restoreInstances: () => Promise<void>;
  addInstance: (
    url: string,
    email: string,
    password: string,
    wsUrl?: string,
    totpCode?: string,
  ) => Promise<void>;
  addValidatedInstance: (config: InstanceConfig) => void;
  removeInstance: (url: string) => void;
  setActiveInstance: (url: string) => void;
  updateInstanceUser: (url: string, user: InstanceUser) => void;
  clearError: () => void;
}

/**
 * The identity provider redirects back to this origin with session cookies and
 * nothing in local storage, so a browser that has never signed in here has no
 * instance to show. Asking the server who we are turns that cookie into the
 * same instance entry a password login would have produced.
 */
async function adoptSsoSession(
  existing: InstanceConfig[],
  store: InstancesState,
): Promise<InstanceConfig | null> {
  const params = new URLSearchParams(window.location.search);
  if (params.get('sso') !== '1') return null;

  params.delete('sso');
  const query = params.toString();
  window.history.replaceState({}, '', `${window.location.pathname}${query ? `?${query}` : ''}`);

  const origin = instanceManager.normalize(window.location.origin);
  if (existing.some((i) => instanceManager.normalize(i.url) === origin)) return null;

  const clients = instanceManager.get(origin);
  try {
    const user = toInstanceUser(await clients.api.typed((c) => c.GET('/users/me')));
    clients.api.onSessionExpired = () => {
      toast.error(ErrorLabels.SessionExpired);
      store.removeInstance(origin);
    };
    clients.ws.onStatusChange = (status) => {
      useWsStatusStore.getState().setStatus(origin, status);
    };
    clients.ws.onSessionRevoked = () => {
      toast.error(ErrorLabels.SessionRevoked);
      store.removeInstance(origin);
    };
    clients.ws.addReconnectListener(backfillAfterReconnect);
    clients.ws.connect();
    return { url: origin, user };
  } catch {
    instanceManager.remove(origin);
    return null;
  }
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
        clients.ws.onSessionRevoked = () => {
          toast.error(ErrorLabels.SessionRevoked);
          get().removeInstance(config.url);
        };
        clients.ws.addReconnectListener(backfillAfterReconnect);

        try {
          const normalized = instanceManager.normalize(config.url);
          if (normalized !== window.location.origin) {
            const tokens = loadTokens(normalized);
            if (!tokens) {
              instanceManager.remove(config.url);
              continue;
            }
            clients.api.onTokensChanged = (access, refresh) => saveTokens(normalized, access, refresh);
            clients.api.setTokens(tokens.access, tokens.refresh);
            const refreshed = await clients.api.refreshSession().catch(() => null);
            if (!refreshed?.user) {
              saveTokens(normalized, null, null);
              instanceManager.remove(config.url);
              continue;
            }
            valid.push({
              url: config.url,
              ...(config.wsUrl ? { wsUrl: config.wsUrl } : {}),
              user: toInstanceUser(refreshed.user),
            });
          } else {
            const user = toInstanceUser(await clients.api.typed((c) => c.GET('/users/me')));
            valid.push({ url: config.url, ...(config.wsUrl ? { wsUrl: config.wsUrl } : {}), user });
          }
          clients.ws.connect();
        } catch {
          instanceManager.remove(config.url);
        }
      }

      const adopted = await adoptSsoSession(valid, get());
      if (adopted) valid.push(adopted);

      saveToStorage(valid);
      set({ instances: valid, activeInstanceUrl: valid[0]?.url ?? null, hydrated: true });
    })();
    return restorePromise;
  },

  addInstance: async (url, email, password, wsUrl?, totpCode?) => {
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
      clients.ws.onSessionRevoked = () => {
        toast.error(ErrorLabels.SessionRevoked);
        get().removeInstance(normalized);
      };
      clients.ws.addReconnectListener(backfillAfterReconnect);

      const res = await clients.api.typed((c) =>
        c.POST('/auth/login', {
          body: { email, password, ...(totpCode ? { totp_code: totpCode } : {}) },
        }),
      );

      if (normalized !== window.location.origin) {
        clients.api.onTokensChanged = (access, refresh) => saveTokens(normalized, access, refresh);
        clients.api.setTokens(res.access_token, res.refresh_token);
      } else {
        clients.api.setTokens(res.access_token, res.refresh_token);
      }
      clients.ws.connect();

      const config: InstanceConfig = {
        url: normalized,
        ...(normalizedWsUrl ? { wsUrl: normalizedWsUrl } : {}),
        user: toInstanceUser(res.user),
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
      .api.typed((c) => c.POST('/auth/logout'))
      .catch(() => {});
    instanceManager.remove(normalized);
    saveTokens(normalized, null, null);
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
