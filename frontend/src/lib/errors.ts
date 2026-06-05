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
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

export function isSessionExpired(error: unknown): boolean {
  return error instanceof ApiError && error.status === 401;
}

export function toUserMessage(error: unknown): string {
  if (error instanceof ApiError || error instanceof HttpError) return error.message;
  if (error instanceof Error && error.message) return error.message;
  return GENERIC_ERROR_MESSAGE;
}
