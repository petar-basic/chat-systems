import {
  Archive,
  BotMessageSquare,
  ChartColumn,
  Clock,
  IdCard,
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
  | 'profile-fields'
  | 'custom-emoji'
  | 'custom-responses'
  | 'apps'
  | 'integrations'
  | 'retention'
  | 'analytics'
  | 'exports'
  | 'slack-import'
  | 'audit-log'
  | 'scim';

type Audience = 'everyone' | 'members' | 'admin' | 'owner' | 'instance-admin';

export interface Viewer {
  role: WorkspaceRole | null;
  isInstanceAdmin: boolean;
}

const SECTIONS: (Omit<ModalSection<SettingsSection>, 'label'> & { audience: Audience })[] = [
  { key: 'general', Icon: Settings, audience: 'admin' },
  { key: 'members', Icon: Users, audience: 'everyone' },
  { key: 'user-groups', Icon: UsersRound, audience: 'everyone' },
  { key: 'profile-fields', Icon: IdCard, audience: 'admin' },
  { key: 'custom-emoji', Icon: Smile, audience: 'everyone' },
  { key: 'custom-responses', Icon: BotMessageSquare, audience: 'members' },
  { key: 'apps', Icon: LayoutGrid, audience: 'everyone' },
  { key: 'integrations', Icon: Plug, audience: 'admin' },
  { key: 'retention', Icon: Clock, audience: 'owner' },
  { key: 'analytics', Icon: ChartColumn, audience: 'admin' },
  { key: 'exports', Icon: Archive, audience: 'admin' },
  { key: 'slack-import', Icon: Upload, audience: 'admin' },
  { key: 'audit-log', Icon: ScrollText, audience: 'admin' },
  { key: 'scim', Icon: KeyRound, audience: 'instance-admin' },
];

export function isWorkspaceAdmin(role: WorkspaceRole | null) {
  return role === 'admin' || role === 'owner';
}

export function canDeleteWorkspace(viewer: Viewer) {
  return viewer.role === 'owner' || viewer.isInstanceAdmin;
}

function sees(viewer: Viewer, audience: Audience) {
  switch (audience) {
    case 'everyone':
      return true;
    case 'members':
      return viewer.role !== null && viewer.role !== 'guest';
    case 'admin':
      return isWorkspaceAdmin(viewer.role);
    case 'owner':
      return viewer.role === 'owner';
    case 'instance-admin':
      return viewer.isInstanceAdmin;
  }
}

export function sectionsFor(viewer: Viewer): Omit<ModalSection<SettingsSection>, 'label'>[] {
  return SECTIONS.filter((s) => sees(viewer, s.audience)).map(({ key, Icon }) => ({ key, Icon }));
}

export function isSettingsSection(value: string | null): value is SettingsSection {
  return SECTIONS.some((s) => s.key === value);
}
