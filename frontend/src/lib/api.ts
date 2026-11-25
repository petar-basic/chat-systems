import createClient, { type Client } from 'openapi-fetch';
import type { components, paths } from '@/api/schema';
import { useMediaTokenStore } from '@/stores/mediaTokens';
import { useServerSupportStore } from '@/stores/serverSupport';
import { API_PATH } from './apiVersion';
import { clientVersion } from './appVersion';
import { ApiError, retryAfterSeconds } from './errors';
import i18n from './i18n';

export interface TypedResult<D> {
  data?: D;
  error?: unknown;
  response: Response;
}

export interface TypedOptions {
  anonymous?: boolean;
}

function errorMessage(error: unknown, response: Response): string {
  if (error && typeof error === 'object') {
    const body = error as { error?: unknown; message?: unknown };
    if (typeof body.error === 'string') return body.error;
    if (typeof body.message === 'string') return body.message;
  }
  return response.statusText;
}

interface RefreshData {
  user?: components['schemas']['UserPublic'];
  access_token?: string;
  refresh_token?: string;
}

function isTokenExpiring(token: string): boolean {
  try {
    const part = token.split('.')[1];
    const payload = JSON.parse(atob(part.replace(/-/g, '+').replace(/_/g, '/'))) as { exp?: number };
    if (typeof payload.exp !== 'number') return false;
    return Date.now() >= payload.exp * 1000 - 10_000;
  } catch {
    return false;
  }
}

const MEDIA_RENEW_BEFORE_SECONDS = 300;
const MEDIA_RETRY_MS = 30_000;
const REFRESH_TIMEOUT_MS = 15_000;

const VERSION_HEADER: Record<string, string> = clientVersion ? { 'X-Client-Version': clientVersion } : {};

function minClientVersion(error: unknown): string {
  const body = error as { min_client_version?: unknown } | null;
  return typeof body?.min_client_version === 'string' ? body.min_client_version : '';
}

export class ApiClient {
  private baseUrl: string;
  private isCrossOrigin: boolean;
  private instanceUrl: string;
  private ageCheck: Promise<boolean> | null = null;

  private memoryToken: string | null = null;
  private refreshToken: string | null = null;

  private refreshPromise: Promise<RefreshData | null> | null = null;
  private typedClient: Client<paths> | null = null;

  private mediaRun = 0;
  private mediaTimer: ReturnType<typeof setTimeout> | null = null;

  onSessionExpired: (() => void) | null = null;
  onTokensChanged: ((access: string | null, refresh: string | null) => void) | null = null;

  constructor(instanceUrl?: string) {
    if (!instanceUrl || instanceUrl === window.location.origin) {
      this.baseUrl = API_PATH;
      this.isCrossOrigin = false;
      this.instanceUrl = window.location.origin;
    } else {
      this.baseUrl = `${instanceUrl}${API_PATH}`;
      this.isCrossOrigin = true;
      this.instanceUrl = instanceUrl;
    }
  }

  private get credentials(): RequestCredentials {
    return this.isCrossOrigin ? 'omit' : 'include';
  }

  private refused(error: unknown): ApiError {
    const minVersion = minClientVersion(error);
    useServerSupportStore.getState().report(this.instanceUrl, { kind: 'needsNewerApp', minVersion });
    return new ApiError(426, i18n.t('common:serverSupport.needsNewerAppError', { version: minVersion }));
  }

  private serverIsTooOld(): Promise<boolean> {
    this.ageCheck ??= fetch(`${this.instanceUrl}/api/instance/info`)
      .then((res) => {
        if (res.ok) useServerSupportStore.getState().report(this.instanceUrl, { kind: 'serverTooOld' });
        else this.ageCheck = null;
        return res.ok;
      })
      .catch(() => {
        this.ageCheck = null;
        return false;
      });
    return this.ageCheck;
  }

  private async unreachable(cause: unknown): Promise<unknown> {
    if (this.isCrossOrigin && (await this.serverIsTooOld())) {
      return new ApiError(0, i18n.t('common:serverSupport.serverTooOldError'));
    }
    return cause;
  }

  private async failure(status: number, error: unknown, response: Response): Promise<ApiError> {
    if (status === 426) return this.refused(error);
    if (status === 404 && !error && (await this.serverIsTooOld())) {
      return new ApiError(404, i18n.t('common:serverSupport.serverTooOldError'));
    }
    return new ApiError(status, errorMessage(error, response), retryAfterSeconds(response));
  }

  setTokens(access: string | null, refresh: string | null) {
    this.memoryToken = access;
    this.refreshToken = refresh;
    this.onTokensChanged?.(access, refresh);
  }

  getAccessToken(): string | null {
    return this.memoryToken;
  }

  async getValidToken(): Promise<string | null> {
    if (this.refreshPromise) {
      await this.refreshPromise;
    }
    if (this.memoryToken && !isTokenExpiring(this.memoryToken)) {
      return this.memoryToken;
    }
    if (!(await this.refresh())) {
      this.onSessionExpired?.();
      return null;
    }
    return this.memoryToken;
  }

  private get client(): Client<paths> {
    if (!this.typedClient) {
      const client = createClient<paths>({ baseUrl: this.baseUrl, credentials: this.credentials });
      client.use({
        onRequest: ({ request }) => {
          for (const [name, value] of Object.entries(this.headers())) {
            request.headers.set(name, value);
          }
          return request;
        },
      });
      this.typedClient = client;
    }
    return this.typedClient;
  }

  typed<D>(
    op: (client: Client<paths>) => Promise<TypedResult<D>>,
    { anonymous = false }: TypedOptions = {},
  ): Promise<D> {
    return this.send(op, !anonymous);
  }

  private async send<D>(
    op: (client: Client<paths>) => Promise<TypedResult<D>>,
    mayRefresh: boolean,
  ): Promise<D> {
    const { data, error, response } = await op(this.client).catch(async (cause: unknown) => {
      throw await this.unreachable(cause);
    });

    if (response.status === 401 && mayRefresh) {
      const refreshed = await this.refresh();
      if (refreshed) {
        return this.send(op, false);
      }
      this.onSessionExpired?.();
      throw new ApiError(401, 'Session expired. Please log in again.');
    }

    if (!response.ok) {
      throw await this.failure(response.status, error, response);
    }

    return data as D;
  }

  private headers(): Record<string, string> {
    return { ...VERSION_HEADER, ...this.getAuthHeaders() };
  }

  private getAuthHeaders(): Record<string, string> {
    if (this.isCrossOrigin && this.memoryToken) {
      return { Authorization: `Bearer ${this.memoryToken}` };
    }
    return {};
  }

  private async refresh(): Promise<boolean> {
    return (await this.refreshSession()) !== null;
  }

  refreshSession(): Promise<RefreshData | null> {
    this.refreshPromise ??= this.performRefresh().finally(() => {
      this.refreshPromise = null;
    });
    return this.refreshPromise;
  }

  private async performRefresh(): Promise<RefreshData | null> {
    const res = await fetch(`${this.baseUrl}/auth/refresh`, {
      method: 'POST',
      credentials: this.credentials,
      signal: AbortSignal.timeout(REFRESH_TIMEOUT_MS),
      headers: {
        ...VERSION_HEADER,
        ...(this.isCrossOrigin && this.refreshToken ? { Authorization: `Bearer ${this.refreshToken}` } : {}),
      },
    });
    if (res.status === 401) return null;
    if (!res.ok) throw await this.failure(res.status, await res.json().catch(() => null), res);

    const data = (await res.json().catch(() => null)) as RefreshData | null;
    if (data?.access_token) {
      this.setTokens(
        data.access_token,
        this.isCrossOrigin ? (data.refresh_token ?? this.refreshToken) : null,
      );
    }
    return data;
  }

  async signOut(): Promise<void> {
    this.stopMediaLinks();
    const refresh = this.isCrossOrigin ? this.refreshToken : null;
    if (this.isCrossOrigin && !refresh) return;
    await fetch(`${this.baseUrl}/auth/logout`, {
      method: 'POST',
      credentials: this.credentials,
      headers: { ...VERSION_HEADER, ...(refresh ? { Authorization: `Bearer ${refresh}` } : {}) },
    }).catch(() => undefined);
  }

  async keepMediaLinks(): Promise<void> {
    if (!this.isCrossOrigin) return;
    const run = ++this.mediaRun;
    if (this.mediaTimer) clearTimeout(this.mediaTimer);
    let renewIn = MEDIA_RETRY_MS;
    try {
      const { token, expires_in } = await this.typed((c) => c.POST('/auth/media-token'));
      if (run !== this.mediaRun) return;
      useMediaTokenStore.getState().put(this.instanceUrl, token);
      renewIn = Math.max(expires_in - MEDIA_RENEW_BEFORE_SECONDS, 60) * 1000;
    } catch {
      if (run !== this.mediaRun) return;
    }
    this.mediaTimer = setTimeout(() => void this.keepMediaLinks(), renewIn);
  }

  stopMediaLinks(): void {
    this.mediaRun++;
    if (this.mediaTimer) clearTimeout(this.mediaTimer);
    this.mediaTimer = null;
    useMediaTokenStore.getState().drop(this.instanceUrl);
  }

  async upload<T>(path: string, formData: FormData, isRetry = false): Promise<T> {
    const res = await fetch(`${this.baseUrl}${path}`, {
      method: 'POST',
      headers: this.headers(),
      credentials: this.credentials,
      body: formData,
    }).catch(async (cause: unknown) => {
      throw await this.unreachable(cause);
    });

    if (res.status === 401 && !isRetry) {
      const refreshed = await this.refresh();
      if (refreshed) return this.upload<T>(path, formData, true);
      this.onSessionExpired?.();
      throw new ApiError(401, 'Session expired. Please log in again.');
    }
    if (!res.ok) {
      const err = await res.json().catch(() => null);
      throw await this.failure(res.status, err, res);
    }
    if (res.status === 204) return undefined as T;
    return res.json() as Promise<T>;
  }

  async uploadWithProgress<T>(
    path: string,
    formData: FormData,
    onProgress: (fraction: number) => void,
    signal?: AbortSignal,
    isRetry = false,
  ): Promise<T> {
    const { status, body } = await new Promise<{ status: number; body: unknown }>((resolve, reject) => {
      const xhr = new XMLHttpRequest();
      xhr.open('POST', `${this.baseUrl}${path}`);
      xhr.withCredentials = !this.isCrossOrigin;
      xhr.responseType = 'json';
      for (const [name, value] of Object.entries(this.headers())) xhr.setRequestHeader(name, value);
      xhr.upload.onprogress = (event) => {
        if (event.lengthComputable) onProgress(event.loaded / event.total);
      };
      xhr.onload = () => resolve({ status: xhr.status, body: xhr.response });
      xhr.onerror = () => reject(new ApiError(0, 'The connection dropped'));
      xhr.onabort = () => reject(new DOMException('Upload canceled', 'AbortError'));
      signal?.addEventListener('abort', () => xhr.abort(), { once: true });
      xhr.send(formData);
    });

    if (status === 401 && !isRetry) {
      const refreshed = await this.refresh();
      if (refreshed) return this.uploadWithProgress<T>(path, formData, onProgress, signal, true);
      this.onSessionExpired?.();
      throw new ApiError(401, 'Session expired. Please log in again.');
    }
    if (status === 426) throw this.refused(body);
    if (status < 200 || status >= 300) {
      const err = body as { error?: string; message?: string } | null;
      throw new ApiError(
        status,
        err?.error ?? err?.message ?? (status >= 500 ? 'The server did not answer' : 'Upload failed'),
      );
    }
    return body as T;
  }
}

export const api = new ApiClient();
