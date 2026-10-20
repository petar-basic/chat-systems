import { GENERIC_ERROR_MESSAGE } from '@/shared/constants';

export class HttpError extends Error {
  constructor(
    public readonly status: number,
    message: string,
  ) {
    super(message);
    this.name = 'HttpError';
  }
}

export class ApiError extends Error {
  constructor(
    public readonly status: number,
    message: string,
    public readonly retryAfterSeconds: number | null = null,
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

export function retryAfterSeconds(response: Response): number | null {
  if (response.status !== 429) return null;
  const raw = response.headers.get('Retry-After');
  const seconds = raw ? Number.parseInt(raw, 10) : Number.NaN;
  return Number.isFinite(seconds) && seconds > 0 ? seconds : null;
}

export function isRateLimited(error: unknown): error is ApiError {
  return error instanceof ApiError && error.status === 429;
}

export function isSessionExpired(error: unknown): boolean {
  return error instanceof ApiError && error.status === 401;
}

/// The login endpoint answers a password that was right but incomplete with a
/// conflict rather than a rejection, so the form can ask for the second factor
/// instead of telling somebody their password was wrong.
export function isTotpRequired(error: unknown): boolean {
  return error instanceof ApiError && error.status === 409 && error.message === 'totp_required';
}

export function toUserMessage(error: unknown, fallback = GENERIC_ERROR_MESSAGE): string {
  if (isRateLimited(error) && error.retryAfterSeconds) {
    return `${error.message} Try again in ${error.retryAfterSeconds}s.`;
  }
  if (error instanceof ApiError || error instanceof HttpError) return error.message;
  if (error instanceof Error && error.message) return error.message;
  return fallback;
}
