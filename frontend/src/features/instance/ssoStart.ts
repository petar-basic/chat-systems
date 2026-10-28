export function ssoStartUrl(invite?: string | null) {
  const start = `${window.location.origin}/api/auth/oidc/start`;
  return invite ? `${start}?invite=${encodeURIComponent(invite)}` : start;
}
