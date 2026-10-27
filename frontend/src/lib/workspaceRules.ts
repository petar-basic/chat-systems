import type { components } from '@/api/schema';

export type WhoMay = components['schemas']['WhoMay'];

export type WorkspaceRule =
  'workflow_builders' | 'broadcast_mentions' | 'public_channel_creators' | 'private_channel_creators';

export function ruleOf(settings: unknown, rule: WorkspaceRule): WhoMay {
  if (typeof settings !== 'object' || settings === null) return 'members';
  return (settings as Record<string, unknown>)[rule] === 'admins' ? 'admins' : 'members';
}

export function allows(who: WhoMay, role: string | null): boolean {
  if (!role || role === 'guest') return false;
  return who === 'members' || role === 'admin' || role === 'owner';
}
