import { describe, it, expect } from 'vitest';
import { ApiError, HttpError, isSessionExpired, isTotpRequired, toUserMessage } from './errors';

describe('isTotpRequired', () => {
  it('separates an incomplete login from a wrong one', () => {
    expect(isTotpRequired(new ApiError(409, 'totp_required'))).toBe(true);
    expect(isTotpRequired(new ApiError(401, 'Invalid credentials'))).toBe(false);
    expect(isTotpRequired(new ApiError(409, 'Email already registered'))).toBe(false);
    expect(isTotpRequired(new HttpError(409, 'totp_required'))).toBe(false);
    expect(isTotpRequired(new Error('totp_required'))).toBe(false);
  });

  it('is not the session-expired path', () => {
    const error = new ApiError(409, 'totp_required');
    expect(isSessionExpired(error)).toBe(false);
    expect(toUserMessage(error)).toBe('totp_required');
  });
});
