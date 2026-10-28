import { describe, it, expect } from 'vitest';
import { ssoStartUrl } from './ssoStart';

describe('ssoStartUrl', () => {
  it('carries the invite through the sign-in so the join can finish after it', () => {
    expect(ssoStartUrl('a.b/c')).toBe(`${window.location.origin}/api/auth/oidc/start?invite=a.b%2Fc`);
  });

  it('starts a plain sign-in without one', () => {
    expect(ssoStartUrl(null)).toBe(`${window.location.origin}/api/auth/oidc/start`);
  });
});
