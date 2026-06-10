import { ApiError } from './errors';

interface RefreshData {
  user?: unknown;
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

  onSessionExpired: (() => void) | null = null;
  onTokensChanged: ((access: string | null, refresh: string | null) => void) | null = null;
  refreshHandler: (() => Promise<RefreshData | null>) | null = null;

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

  private getAuthHeaders(): Record<string, string> {
    if (this.isCrossOrigin && this.memoryToken) {
      return { Authorization: `Bearer ${this.memoryToken}` };
    }
    return {};
  }

  private async request<T>(method: string, path: string, body?: unknown, isRetry = false): Promise<T> {
    const res = await fetch(`${this.baseUrl}${path}`, {
      method,
      headers: {
        'Content-Type': 'application/json',
        ...this.getAuthHeaders(),
      },
      credentials: 'include',
      body: body ? JSON.stringify(body) : undefined,
    });

    if (res.status === 401 && !isRetry) {
      const refreshed = await this.refresh();
      if (refreshed) {
        return this.request<T>(method, path, body, true);
      }
      this.onSessionExpired?.();
      throw new ApiError(401, 'Session expired. Please log in again.');
    }

    if (!res.ok) {
      const err = await res.json().catch(() => ({ error: res.statusText }));
      throw new ApiError(res.status, err.error || err.message || res.statusText);
    }

    if (res.status === 204) return undefined as T;
    return res.json() as Promise<T>;
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
    if (this.refreshHandler) {
      const data = await this.refreshHandler().catch(() => null);
      if (data?.access_token) this.setTokens(data.access_token, null);
      return data;
    }
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
      throw new ApiError(res.status, err.error || err.message || res.statusText);
    }
    if (res.status === 204) return undefined as T;
    return res.json() as Promise<T>;
  }

  get<T>(path: string) {
    return this.request<T>('GET', path);
  }
  post<T>(path: string, body?: unknown) {
    return this.request<T>('POST', path, body);
  }
  patch<T>(path: string, body?: unknown) {
    return this.request<T>('PATCH', path, body);
  }
  delete<T>(path: string) {
    return this.request<T>('DELETE', path);
  }
}

export const api = new ApiClient();
