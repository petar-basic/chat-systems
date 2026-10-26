import { useState } from 'react';
import type { Channel, Workspace, WorkspaceMember } from '@/stores/workspace';
import { SectionedModal } from '@/shared/components/Modal/SectionedModal';
import { sectionsFor, type SettingsSection } from './sections';
import { GeneralSection } from './GeneralSection';
import { MembersSection } from './MembersSection';
import { UserGroupsSection } from './UserGroupsSection';
import { CustomEmojiSection } from './CustomEmojiSection';
import { AppsSection } from './AppsSection';
import { IntegrationsSection } from './IntegrationsSection';
import { SlackImportSection } from './SlackImportSection';
import { AuditLogSection } from './AuditLogSection';

interface Props {
  workspace: Workspace;
  isAdmin: boolean;
  initial: SettingsSection;
  inviting?: boolean;
  members: WorkspaceMember[];
  channels: Channel[];
  onClose: () => void;
}

export function WorkspaceSettingsModal({
  workspace,
  isAdmin,
  initial,
  inviting = false,
  members,
  channels,
  onClose,
}: Props) {
  const sections = sectionsFor(isAdmin);
  const [active, setActive] = useState<SettingsSection>(
    sections.some((s) => s.key === initial) ? initial : sections[0].key,
  );
  const { id, instanceUrl } = workspace;

  return (
    <SectionedModal
      title="Workspace settings"
      sections={sections}
      active={active}
      onSelect={setActive}
      onClose={onClose}
      size="full"
      qaPrefix="workspace-settings"
    >
      {active === 'general' && <GeneralSection workspace={workspace} onDeleted={onClose} />}
      {active === 'members' && <MembersSection workspaceId={id} openInvite={inviting} />}
      {active === 'user-groups' && (
        <UserGroupsSection workspaceId={id} instanceUrl={instanceUrl} members={members} isAdmin={isAdmin} />
      )}
      {active === 'custom-emoji' && <CustomEmojiSection workspaceId={id} instanceUrl={instanceUrl} />}
      {active === 'apps' && <AppsSection workspaceId={id} isAdmin={isAdmin} />}
      {active === 'integrations' && (
        <IntegrationsSection workspaceId={id} instanceUrl={instanceUrl} channels={channels} />
      )}
      {active === 'slack-import' && <SlackImportSection workspaceId={id} instanceUrl={instanceUrl} />}
      {active === 'audit-log' && <AuditLogSection workspaceId={id} instanceUrl={instanceUrl} />}
    </SectionedModal>
  );
}
