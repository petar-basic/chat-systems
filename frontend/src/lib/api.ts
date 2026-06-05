import { ApiError } from './errors';

export class ApiClient {
  private baseUrl: string;
  private isCrossOrigin: boolean;

  private memoryToken: string | null = null;

  private refreshPromise: Promise<boolean> | null = null;

  onSessionExpired: (() => void) | null = null;

  constructor(instanceUrl?: string) {
    if (!instanceUrl || instanceUrl === window.location.origin) {
      this.baseUrl = '/api';
      this.isCrossOrigin = false;
    } else {
      this.baseUrl = `${instanceUrl}/api`;
      this.isCrossOrigin = true;
    }
  }

  setToken(token: string | null) {
    this.memoryToken = token;
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
    this.refreshPromise = this.doRefresh().finally(() => {
      this.refreshPromise = null;
    });
    return this.refreshPromise;
  }

  private async doRefresh(): Promise<boolean> {
    try {
      const res = await fetch(`${this.baseUrl}/auth/refresh`, {
        method: 'POST',
        credentials: 'include',
        headers: this.memoryToken ? { Authorization: `Bearer ${this.memoryToken}` } : {},
      });
      if (!res.ok) return false;

      if (this.isCrossOrigin) {
        const data = await res.json().catch(() => null);
        if (data?.access_token) {
          this.memoryToken = data.access_token;
        }
      }
      return true;
    } catch {
      return false;
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
