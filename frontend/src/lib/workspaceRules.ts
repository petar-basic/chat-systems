import type { components } from '@/api/schema';

export type WhoMay = components['schemas']['WhoMay'];

export type WorkspaceRule =
  | 'workflow_builders'
  | 'broadcast_mentions'
  | 'public_channel_creators'
  | 'private_channel_creators'
  | 'inviters';

const UNSET: Record<WorkspaceRule, WhoMay> = {
  workflow_builders: 'members',
  broadcast_mentions: 'members',
  public_channel_creators: 'members',
  private_channel_creators: 'members',
  inviters: 'admins',
};

export function ruleOf(settings: unknown, rule: WorkspaceRule): WhoMay {
  if (typeof settings !== 'object' || settings === null) return UNSET[rule];
  const value = (settings as Record<string, unknown>)[rule];
  return value === 'admins' || value === 'members' ? value : UNSET[rule];
}

export function allows(who: WhoMay, role: string | null): boolean {
  if (!role || role === 'guest') return false;
  return who === 'members' || role === 'admin' || role === 'owner';
}
