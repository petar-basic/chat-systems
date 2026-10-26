import { ScrollText, Settings, Smile, Upload, Users, UsersRound, LayoutGrid, Plug } from 'lucide-react';
import type { ModalSection } from '@/shared/components/Modal/SectionedModal';

export type SettingsSection =
  | 'general'
  | 'members'
  | 'user-groups'
  | 'custom-emoji'
  | 'apps'
  | 'integrations'
  | 'slack-import'
  | 'audit-log';

const SECTIONS: (ModalSection<SettingsSection> & { adminOnly: boolean })[] = [
  { key: 'general', label: 'General', Icon: Settings, adminOnly: true },
  { key: 'members', label: 'Members', Icon: Users, adminOnly: false },
  { key: 'user-groups', label: 'User groups', Icon: UsersRound, adminOnly: false },
  { key: 'custom-emoji', label: 'Custom emoji', Icon: Smile, adminOnly: false },
  { key: 'apps', label: 'Apps', Icon: LayoutGrid, adminOnly: false },
  { key: 'integrations', label: 'Integrations', Icon: Plug, adminOnly: true },
  { key: 'slack-import', label: 'Slack import', Icon: Upload, adminOnly: true },
  { key: 'audit-log', label: 'Audit log', Icon: ScrollText, adminOnly: true },
];

export function sectionsFor(isAdmin: boolean): ModalSection<SettingsSection>[] {
  return SECTIONS.filter((s) => isAdmin || !s.adminOnly).map(({ key, label, Icon }) => ({
    key,
    label,
    Icon,
  }));
}

export function isSettingsSection(value: string | null): value is SettingsSection {
  return SECTIONS.some((s) => s.key === value);
}
