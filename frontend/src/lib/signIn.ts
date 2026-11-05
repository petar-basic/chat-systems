import { PENDING_INVITE_KEY } from '@/shared/constants';

export const SECOND_FACTOR_ANSWERS = new Set([
  'totp_required',
  'totp_or_passkey_required',
  'passkey_required',
]);

export function afterSignIn(): string {
  const pending = sessionStorage.getItem(PENDING_INVITE_KEY);
  if (!pending) return '/app';
  sessionStorage.removeItem(PENDING_INVITE_KEY);
  return `/invite/${pending}`;
}
