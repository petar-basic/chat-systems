import { KeyRound } from 'lucide-react';
import { useInstanceInfo } from './hooks/useInstanceInfo';
import { ssoStartUrl } from './ssoStart';

export function SsoInviteButton({ token }: { token: string }) {
  const { data: instance } = useInstanceInfo(window.location.origin);
  if (!instance?.sso_enabled) return null;
  return (
    <>
      <div className="flex items-center gap-3 my-6">
        <div className="h-px flex-1 bg-raised" />
        <span className="text-xs uppercase tracking-wide text-subtle">or</span>
        <div className="h-px flex-1 bg-raised" />
      </div>
      <a
        href={ssoStartUrl(token)}
        data-qa="invite-sso-start"
        className="w-full flex items-center justify-center gap-2 px-4 py-3 bg-raised hover:bg-elevated text-fg font-medium rounded-lg transition"
      >
        <KeyRound className="w-4 h-4" />
        Continue with SSO
      </a>
    </>
  );
}
