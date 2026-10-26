import {
  Archive,
  Clock,
  KeyRound,
  LayoutGrid,
  Plug,
  ScrollText,
  Settings,
  Smile,
  Upload,
  Users,
  UsersRound,
} from 'lucide-react';
import type { ModalSection } from '@/shared/components/Modal/SectionedModal';
import type { WorkspaceRole } from '@/stores/workspace';

export type SettingsSection =
  | 'general'
  | 'members'
  | 'user-groups'
  | 'custom-emoji'
  | 'apps'
  | 'integrations'
  | 'retention'
  | 'exports'
  | 'slack-import'
  | 'audit-log'
  | 'scim';

type Audience = 'everyone' | 'admin' | 'owner' | 'instance-admin';

export interface Viewer {
  role: WorkspaceRole | null;
  isInstanceAdmin: boolean;
}

const SECTIONS: (ModalSection<SettingsSection> & { audience: Audience })[] = [
  { key: 'general', label: 'General', Icon: Settings, audience: 'admin' },
  { key: 'members', label: 'Members', Icon: Users, audience: 'everyone' },
  { key: 'user-groups', label: 'User groups', Icon: UsersRound, audience: 'everyone' },
  { key: 'custom-emoji', label: 'Custom emoji', Icon: Smile, audience: 'everyone' },
  { key: 'apps', label: 'Apps', Icon: LayoutGrid, audience: 'everyone' },
  { key: 'integrations', label: 'Integrations', Icon: Plug, audience: 'admin' },
  { key: 'retention', label: 'Data retention', Icon: Clock, audience: 'owner' },
  { key: 'exports', label: 'Exports', Icon: Archive, audience: 'admin' },
  { key: 'slack-import', label: 'Slack import', Icon: Upload, audience: 'admin' },
  { key: 'audit-log', label: 'Audit log', Icon: ScrollText, audience: 'admin' },
  { key: 'scim', label: 'SCIM provisioning', Icon: KeyRound, audience: 'instance-admin' },
];

export function isWorkspaceAdmin(role: WorkspaceRole | null) {
  return role === 'admin' || role === 'owner';
}

function sees(viewer: Viewer, audience: Audience) {
  switch (audience) {
    case 'everyone':
      return true;
    case 'admin':
      return isWorkspaceAdmin(viewer.role);
    case 'owner':
      return viewer.role === 'owner';
    case 'instance-admin':
      return viewer.isInstanceAdmin;
  }
}

export function sectionsFor(viewer: Viewer): ModalSection<SettingsSection>[] {
  return SECTIONS.filter((s) => sees(viewer, s.audience)).map(({ key, label, Icon }) => ({
    key,
    label,
    Icon,
  }));
}

export function isSettingsSection(value: string | null): value is SettingsSection {
  return SECTIONS.some((s) => s.key === value);
}
