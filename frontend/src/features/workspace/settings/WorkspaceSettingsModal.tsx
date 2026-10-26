import { useState } from 'react';
import type { Channel, Workspace, WorkspaceMember } from '@/stores/workspace';
import { SectionedModal } from '@/shared/components/Modal/SectionedModal';
import { isWorkspaceAdmin, sectionsFor, type SettingsSection, type Viewer } from './sections';
import { GeneralSection } from './GeneralSection';
import { MembersSection } from './MembersSection';
import { UserGroupsSection } from './UserGroupsSection';
import { CustomEmojiSection } from './CustomEmojiSection';
import { AppsSection } from './AppsSection';
import { IntegrationsSection } from './IntegrationsSection';
import { SlackImportSection } from './SlackImportSection';
import { AuditLogSection } from './AuditLogSection';
import { RetentionSection } from './RetentionSection';
import { ExportsSection } from './ExportsSection';
import { ScimSection } from './ScimSection';

interface Props {
  workspace: Workspace;
  viewer: Viewer;
  initial: SettingsSection;
  inviting?: boolean;
  members: WorkspaceMember[];
  channels: Channel[];
  onClose: () => void;
}

export function WorkspaceSettingsModal({
  workspace,
  viewer,
  initial,
  inviting = false,
  members,
  channels,
  onClose,
}: Props) {
  const sections = sectionsFor(viewer);
  const isAdmin = isWorkspaceAdmin(viewer.role);
  const [chosen, setChosen] = useState<SettingsSection>(initial);
  const active = sections.some((s) => s.key === chosen) ? chosen : sections[0].key;
  const { id, instanceUrl } = workspace;

  return (
    <SectionedModal
      title="Workspace settings"
      sections={sections}
      active={active}
      onSelect={setChosen}
      onClose={onClose}
      size="full"
      qaPrefix="workspace-settings"
    >
      {active === 'general' && <GeneralSection workspace={workspace} onDeleted={onClose} />}
      {active === 'members' && (
        <MembersSection workspaceId={id} openInvite={inviting} isInstanceAdmin={viewer.isInstanceAdmin} />
      )}
      {active === 'user-groups' && (
        <UserGroupsSection workspaceId={id} instanceUrl={instanceUrl} members={members} isAdmin={isAdmin} />
      )}
      {active === 'custom-emoji' && <CustomEmojiSection workspaceId={id} instanceUrl={instanceUrl} />}
      {active === 'apps' && <AppsSection workspaceId={id} isAdmin={isAdmin} />}
      {active === 'integrations' && (
        <IntegrationsSection workspaceId={id} instanceUrl={instanceUrl} channels={channels} />
      )}
      {active === 'retention' && <RetentionSection workspaceId={id} instanceUrl={instanceUrl} />}
      {active === 'exports' && (
        <ExportsSection
          workspaceId={id}
          instanceUrl={instanceUrl}
          isOwner={viewer.role === 'owner'}
          members={members}
        />
      )}
      {active === 'slack-import' && <SlackImportSection workspaceId={id} instanceUrl={instanceUrl} />}
      {active === 'audit-log' && <AuditLogSection workspaceId={id} instanceUrl={instanceUrl} />}
      {active === 'scim' && <ScimSection instanceUrl={instanceUrl} />}
    </SectionedModal>
  );
}
