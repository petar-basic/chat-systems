import type { Channel } from '@/stores/workspace';

export const SSO_RUNBOOK_URL =
  'https://github.com/petar-basic/chat-systems/blob/main/docs/RUNBOOK.md#single-sign-on';

export type StepKey = 'invite' | 'channel' | 'icon' | 'sso';

export interface Step {
  key: StepKey;
  label: string;
  done: boolean;
}

export interface WorkspaceProgress {
  memberCount: number;
  inviteCount: number;
  channels: Channel[];
  iconUrl: string | null | undefined;
  ssoEnabled: boolean;
}

function hasOwnChannel(channels: Channel[]) {
  return channels.some(
    (ch) => !ch.is_default && (ch.channel_type === 'public' || ch.channel_type === 'private'),
  );
}

export function isGettingStarted(memberCount: number, channels: Channel[]) {
  return memberCount < 3 || !hasOwnChannel(channels);
}

export function checklistSteps(progress: WorkspaceProgress): Step[] {
  return [
    {
      key: 'invite',
      label: 'Invite your team',
      done: progress.memberCount > 1 || progress.inviteCount > 0,
    },
    { key: 'channel', label: 'Create a channel', done: hasOwnChannel(progress.channels) },
    { key: 'icon', label: 'Set an icon', done: !!progress.iconUrl },
    { key: 'sso', label: 'Connect SSO', done: progress.ssoEnabled },
  ];
}

const dismissKey = (workspaceId: string) => `workspace-checklist-dismissed:${workspaceId}`;

export function wasDismissed(workspaceId: string): boolean {
  try {
    return localStorage.getItem(dismissKey(workspaceId)) === '1';
  } catch {
    return false;
  }
}

export function rememberDismissal(workspaceId: string) {
  try {
    localStorage.setItem(dismissKey(workspaceId), '1');
  } catch {
    return;
  }
}
