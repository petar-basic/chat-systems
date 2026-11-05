import { describe, it, expect } from 'vitest';
import {
  acceptsPasskey,
  ApiError,
  HttpError,
  isPasskeyRequired,
  isSessionExpired,
  isTotpRequired,
  refusalOr,
  toUserMessage,
} from './errors';

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

describe('acceptsPasskey', () => {
  it('knows when a passkey may stand in for the code, and when only a passkey will do', () => {
    const either = new ApiError(409, 'totp_or_passkey_required');
    const passkey = new ApiError(409, 'passkey_required');

    expect(isTotpRequired(either)).toBe(true);
    expect(acceptsPasskey(either)).toBe(true);
    expect(isPasskeyRequired(either)).toBe(false);

    expect(isTotpRequired(passkey)).toBe(false);
    expect(acceptsPasskey(passkey)).toBe(true);
    expect(isPasskeyRequired(passkey)).toBe(true);

    expect(acceptsPasskey(new ApiError(409, 'totp_required'))).toBe(false);
  });
});

describe('refusalOr', () => {
  it('says why the server refused, and falls back for anything else', () => {
    const reason = "Only workspace admins and this channel's admins can use @channel, @here and @everyone";
    expect(refusalOr(new ApiError(403, reason), 'Message failed to send')).toBe(reason);
    expect(refusalOr(new ApiError(502, 'Bad gateway'), 'Message failed to send')).toBe(
      'Message failed to send',
    );
    expect(refusalOr(new TypeError('Failed to fetch'), 'Message failed to send')).toBe(
      'Message failed to send',
    );
  });
});
