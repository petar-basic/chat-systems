import createClient, { type Client } from 'openapi-fetch';
import type { components, paths } from '@/api/schema';
import { ApiError, retryAfterSeconds } from './errors';

export interface TypedResult<D> {
  data?: D;
  error?: unknown;
  response: Response;
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

export class ApiClient {
  private baseUrl: string;
  private isCrossOrigin: boolean;

  private memoryToken: string | null = null;
  private refreshToken: string | null = null;

  private refreshPromise: Promise<boolean> | null = null;
  private typedClient: Client<paths> | null = null;

  onSessionExpired: (() => void) | null = null;
  onTokensChanged: ((access: string | null, refresh: string | null) => void) | null = null;

  constructor(instanceUrl?: string) {
    if (!instanceUrl || instanceUrl === window.location.origin) {
      this.baseUrl = '/api';
      this.isCrossOrigin = false;
    } else {
      this.baseUrl = `${instanceUrl}/api`;
      this.isCrossOrigin = true;
    }
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
    await this.refresh();
    return this.memoryToken;
  }

  private get client(): Client<paths> {
    if (!this.typedClient) {
      const client = createClient<paths>({ baseUrl: this.baseUrl, credentials: 'include' });
      client.use({
        onRequest: ({ request }) => {
          for (const [name, value] of Object.entries(this.getAuthHeaders())) {
            request.headers.set(name, value);
          }
          return request;
        },
      });
      this.typedClient = client;
    }
    return this.typedClient;
  }

  async typed<D>(op: (client: Client<paths>) => Promise<TypedResult<D>>, isRetry = false): Promise<D> {
    const { data, error, response } = await op(this.client);

    if (response.status === 401 && !isRetry) {
      const refreshed = await this.refresh();
      if (refreshed) {
        return this.typed(op, true);
      }
      this.onSessionExpired?.();
      throw new ApiError(401, 'Session expired. Please log in again.');
    }

    if (!response.ok) {
      throw new ApiError(response.status, errorMessage(error, response), retryAfterSeconds(response));
    }

    return data as D;
  }

  private getAuthHeaders(): Record<string, string> {
    if (this.isCrossOrigin && this.memoryToken) {
      return { Authorization: `Bearer ${this.memoryToken}` };
    }
    return {};
  }

  private refresh(): Promise<boolean> {
    if (this.refreshPromise) {
      return this.refreshPromise;
    }
    this.refreshPromise = this.performRefresh()
      .then((data) => data !== null)
      .finally(() => {
        this.refreshPromise = null;
      });
    return this.refreshPromise;
  }

  async refreshSession(): Promise<RefreshData | null> {
    return this.performRefresh();
  }

  private async performRefresh(): Promise<RefreshData | null> {
    try {
      const res = await fetch(`${this.baseUrl}/auth/refresh`, {
        method: 'POST',
        credentials: 'include',
        headers:
          this.isCrossOrigin && this.refreshToken ? { Authorization: `Bearer ${this.refreshToken}` } : {},
      });
      if (!res.ok) return null;

      const data = (await res.json().catch(() => null)) as RefreshData | null;
      if (data?.access_token) {
        this.setTokens(
          data.access_token,
          this.isCrossOrigin ? (data.refresh_token ?? this.refreshToken) : null,
        );
      }
      return data;
    } catch {
      return null;
    }
  }

  async upload<T>(path: string, formData: FormData, isRetry = false): Promise<T> {
    const res = await fetch(`${this.baseUrl}${path}`, {
      method: 'POST',
      headers: { ...this.getAuthHeaders() },
      credentials: 'include',
      body: formData,
    });

    if (res.status === 401 && !isRetry) {
      const refreshed = await this.refresh();
      if (refreshed) return this.upload<T>(path, formData, true);
      this.onSessionExpired?.();
      throw new ApiError(401, 'Session expired. Please log in again.');
    }
    if (!res.ok) {
      const err = await res.json().catch(() => ({ error: res.statusText }));
      throw new ApiError(res.status, err.error || err.message || res.statusText, retryAfterSeconds(res));
    }
    if (res.status === 204) return undefined as T;
    return res.json() as Promise<T>;
  }
}

export const api = new ApiClient();
