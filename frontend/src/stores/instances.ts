import { create } from 'zustand';
import { instanceManager, type InstanceClients } from '../lib/instances';
import { otherServersAllowed } from '../lib/instanceInfo';
import { forgetSession, keepSession, loadSession } from '../lib/sessionVault';
import i18n from '../lib/i18n';
import { requestPasskey, wasCancelled } from '../lib/webauthn';
import { useWsStatusStore } from './wsStatus';
import { backfillAfterReconnect, refetchAfterOfflineStart } from '../lib/realtimeBackfill';
import { giveUpOnAnswers } from '../lib/commandAnswers';
import { ApiError } from '../lib/errors';
import { logger } from '../lib/logger';
import { toast } from '@/shared/components/Toast';
import { ErrorLabels } from '@/shared/constants';
import type { components } from '@/api/schema';

export type InstanceRole = components['schemas']['InstanceRole'];

export interface InstanceUser {
  id: string;
  email: string;
  display_name: string;
  avatar_url: string | null;
  instance_role: InstanceRole;
}

export function toInstanceUser(user: components['schemas']['UserPublic']): InstanceUser {
  return {
    id: user.id,
    email: user.email,
    display_name: user.display_name ?? '',
    avatar_url: user.avatar_url ?? null,
    instance_role: user.instance_role,
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

export function currentUserOf(
  state: Pick<InstancesState, 'instances' | 'activeInstanceUrl'>,
): InstanceUser | null {
  const active = state.activeInstanceUrl
    ? state.instances.find((i) => i.url === state.activeInstanceUrl)
    : undefined;
  return active?.user ?? state.instances[0]?.user ?? null;
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
  addInstanceWithPasskey: (url: string, firstFactor?: PasskeySecondFactorStart) => Promise<void>;
  addInstanceWithEmailCode: (url: string, email: string, code: string, totpCode?: string) => Promise<void>;
  addValidatedInstance: (config: InstanceConfig) => void;
  removeInstance: (url: string) => void;
  setActiveInstance: (url: string) => void;
  updateInstanceUser: (url: string, user: InstanceUser) => void;
  refreshInstanceUser: (url: string) => Promise<void>;
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
    clients.ws.addReconnectListener(giveUpOnAnswers);
    clients.ws.connect();
    return { url: origin, user };
  } catch {
    instanceManager.remove(origin);
    return null;
  }
}

type AuthSession = components['schemas']['AuthSession'];
type PasskeySecondFactorStart = components['schemas']['PasskeySecondFactorStart'];
type StoreSet = (partial: Partial<InstancesState>) => void;

async function signInTo(
  set: StoreSet,
  get: () => InstancesState,
  url: string,
  wsUrl: string | undefined,
  signIn: (api: InstanceClients['api']) => Promise<AuthSession>,
) {
  set({ loading: true, error: null });
  const normalized = instanceManager.normalize(url);
  const normalizedWsUrl = wsUrl?.trim() || undefined;
  try {
    if (normalized !== window.location.origin && !(await otherServersAllowed())) {
      throw new Error(i18n.t('auth:signIn.otherServersOff'));
    }
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
    clients.ws.addReconnectListener(giveUpOnAnswers);

    const res = await signIn(clients.api);

    if (normalized !== window.location.origin) {
      clients.api.onTokensChanged = (access, refresh) => void keepSession(normalized, access, refresh);
    }
    clients.api.setTokens(res.access_token, res.refresh_token);
    await clients.api.keepMediaLinks();
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
    const msg = wasCancelled(e) ? null : e instanceof Error ? e.message : 'Failed to connect to instance';
    set({ loading: false, error: msg });
    throw e;
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
      const sockets: InstanceClients['ws'][] = [];
      const origin = instanceManager.normalize(window.location.origin);
      const elsewhere = (config: InstanceConfig) => instanceManager.normalize(config.url) !== origin;
      const othersAllowed = saved.some(elsewhere) ? await otherServersAllowed() : true;

      for (const config of saved) {
        if (!othersAllowed && elsewhere(config)) {
          void forgetSession(instanceManager.normalize(config.url));
          continue;
        }
        if (config.wsUrl) {
          instanceManager.setWsUrl(config.url, config.wsUrl);
        }
        const clients = instanceManager.get(config.url);
        const normalized = instanceManager.normalize(config.url);
        const server = { url: config.url, ...(config.wsUrl ? { wsUrl: config.wsUrl } : {}) };
        let unchecked = false;
        clients.api.onSessionExpired = () => {
          toast.error(ErrorLabels.SessionExpired);
          get().removeInstance(config.url);
        };
        clients.ws.onStatusChange = (status) => {
          useWsStatusStore.getState().setStatus(config.url, status);
          if (status !== 'connected' || !unchecked) return;
          unchecked = false;
          refetchAfterOfflineStart();
          void clients.api.keepMediaLinks();
          get()
            .refreshInstanceUser(config.url)
            .catch((err: unknown) => logger.error('instances', 'refreshInstanceUser', err));
        };
        clients.ws.onSessionRevoked = () => {
          toast.error(ErrorLabels.SessionRevoked);
          get().removeInstance(config.url);
        };
        clients.ws.addReconnectListener(backfillAfterReconnect);
        clients.ws.addReconnectListener(giveUpOnAnswers);

        try {
          if (normalized !== window.location.origin) {
            const refresh = await loadSession(normalized);
            if (!refresh) {
              instanceManager.remove(config.url);
              continue;
            }
            clients.api.setTokens(null, refresh);
            clients.api.onTokensChanged = (access, next) => void keepSession(normalized, access, next);
            const refreshed = await clients.api.refreshSession();
            if (!refreshed?.user) {
              void forgetSession(normalized);
              instanceManager.remove(config.url);
              continue;
            }
            await clients.api.keepMediaLinks();
            valid.push({ ...server, user: toInstanceUser(refreshed.user) });
          } else {
            valid.push({
              ...server,
              user: toInstanceUser(await clients.api.typed((c) => c.GET('/users/me'))),
            });
          }
        } catch (err) {
          if (err instanceof ApiError && err.status === 401) {
            instanceManager.remove(config.url);
            continue;
          }
          unchecked = true;
          valid.push({ ...server, user: config.user });
        }
        sockets.push(clients.ws);
      }

      const adopted = await adoptSsoSession(valid, get());
      if (adopted) valid.push(adopted);

      saveToStorage(valid);
      set({ instances: valid, activeInstanceUrl: valid[0]?.url ?? null, hydrated: true });
      for (const ws of sockets) ws.connect();
    })();
    return restorePromise;
  },

  addInstance: async (url, email, password, wsUrl?, totpCode?) => {
    await signInTo(set, get, url, wsUrl, (api) =>
      api.typed(
        (c) =>
          c.POST('/auth/login', {
            body: { email, password, ...(totpCode ? { totp_code: totpCode } : {}) },
          }),
        { anonymous: true },
      ),
    );
  },

  addInstanceWithPasskey: async (url, firstFactor?) => {
    await signInTo(set, get, url, undefined, async (api) => {
      if (firstFactor) {
        const challenge = await api.typed(
          (c) => c.POST('/auth/passkeys/second-factor/start', { body: firstFactor }),
          {
            anonymous: true,
          },
        );
        const credential = await requestPasskey(challenge.options);
        return api.typed(
          (c) =>
            c.POST('/auth/passkeys/second-factor', {
              body: { challenge_id: challenge.challenge_id, credential },
            }),
          { anonymous: true },
        );
      }
      const challenge = await api.typed((c) => c.POST('/auth/passkeys/sign-in/start'), { anonymous: true });
      const credential = await requestPasskey(challenge.options);
      return api.typed(
        (c) =>
          c.POST('/auth/passkeys/sign-in', {
            body: { challenge_id: challenge.challenge_id, credential },
          }),
        { anonymous: true },
      );
    });
  },

  addInstanceWithEmailCode: async (url, email, code, totpCode?) => {
    await signInTo(set, get, url, undefined, (api) =>
      api.typed(
        (c) =>
          c.POST('/auth/email-code/verify', {
            body: { email, code, ...(totpCode ? { totp_code: totpCode } : {}) },
          }),
        { anonymous: true },
      ),
    );
  },

  addValidatedInstance: (config) => {
    const existing = get().instances.filter((i) => i.url !== config.url);
    const instances = [...existing, config];
    saveToStorage(instances);
    set({ instances, activeInstanceUrl: config.url });
  },

  removeInstance: (url) => {
    const normalized = instanceManager.normalize(url);
    void instanceManager.get(normalized).api.signOut();
    instanceManager.remove(normalized);
    void forgetSession(normalized);
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

  refreshInstanceUser: async (url) => {
    const normalized = instanceManager.normalize(url);
    if (!get().instances.some((i) => i.url === normalized)) return;
    const { api } = instanceManager.get(normalized);
    const refreshed = await api.refreshSession();
    const user = refreshed?.user ?? (await api.typed((c) => c.GET('/users/me')));
    get().updateInstanceUser(normalized, toInstanceUser(user));
  },

  clearError: () => set({ error: null }),
}));
