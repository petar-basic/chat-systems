import { describe, it, expect, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/react';
import { WorkspaceSettingsModal } from './WorkspaceSettingsModal';
import type { Workspace, WorkspaceRole } from '@/stores/workspace';

vi.mock('./GeneralSection', () => ({ GeneralSection: () => <p>general body</p> }));
vi.mock('./MembersSection', () => ({
  MembersSection: ({ openInvite }: { openInvite: boolean }) => <p>members body {String(openInvite)}</p>,
}));
vi.mock('./UserGroupsSection', () => ({ UserGroupsSection: () => <p>groups body</p> }));
vi.mock('./CustomEmojiSection', () => ({ CustomEmojiSection: () => <p>emoji body</p> }));
vi.mock('./AppsSection', () => ({ AppsSection: () => <p>apps body</p> }));
vi.mock('./IntegrationsSection', () => ({ IntegrationsSection: () => <p>integrations body</p> }));
vi.mock('./SlackImportSection', () => ({ SlackImportSection: () => <p>import body</p> }));
vi.mock('./AuditLogSection', () => ({ AuditLogSection: () => <p>audit body</p> }));
vi.mock('./RetentionSection', () => ({ RetentionSection: () => <p>retention body</p> }));
vi.mock('./ExportsSection', () => ({ ExportsSection: () => <p>exports body</p> }));
vi.mock('./ScimSection', () => ({ ScimSection: () => <p>scim body</p> }));

const workspace = { id: 'ws-1', name: 'Dev Team', instanceUrl: 'http://localhost:8080' } as Workspace;

function renderModal(
  role: WorkspaceRole,
  initial: Parameters<typeof WorkspaceSettingsModal>[0]['initial'],
  isInstanceAdmin = false,
) {
  render(
    <WorkspaceSettingsModal
      workspace={workspace}
      viewer={{ role, isInstanceAdmin }}
      initial={initial}
      members={[]}
      channels={[]}
      onClose={vi.fn()}
    />,
  );
}

const tabNames = () => screen.getAllByRole('tab').map((t) => t.textContent);

describe('WorkspaceSettingsModal', () => {
  it('gives an admin every workspace section but retention, in one place', () => {
    renderModal('admin', 'general');
    expect(tabNames()).toEqual([
      'General',
      'Members',
      'User groups',
      'Custom emoji',
      'Apps',
      'Integrations',
      'Exports',
      'Slack import',
      'Audit log',
    ]);
    expect(screen.getByText('general body')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('tab', { name: 'Audit log' }));
    expect(screen.getByText('audit body')).toBeInTheDocument();
  });

  it('gives a member only what they can use', () => {
    renderModal('member', 'members');
    expect(tabNames()).toEqual(['Members', 'User groups', 'Custom emoji', 'Apps']);
  });

  it('falls back to the first section a member may open', () => {
    renderModal('member', 'audit-log');
    expect(screen.getByRole('tab', { name: 'Members' })).toHaveAttribute('aria-selected', 'true');
    expect(screen.queryByText('audit body')).not.toBeInTheDocument();
  });

  it('gives the owner data retention, as the API does', () => {
    renderModal('owner', 'retention');
    expect(tabNames()).toContain('Data retention');
    expect(screen.getByText('retention body')).toBeInTheDocument();
  });

  it('keeps SCIM for instance admins, whatever their workspace role', () => {
    renderModal('owner', 'scim');
    expect(tabNames()).not.toContain('SCIM provisioning');

    cleanup();
    renderModal('member', 'scim', true);
    expect(tabNames()).toContain('SCIM provisioning');
    expect(screen.getByText('scim body')).toBeInTheDocument();
  });

  it('opens the linked section once the role it needs is known', () => {
    const props = {
      workspace,
      initial: 'retention' as const,
      members: [],
      channels: [],
      onClose: vi.fn(),
    };
    const { rerender } = render(
      <WorkspaceSettingsModal {...props} viewer={{ role: null, isInstanceAdmin: false }} />,
    );
    expect(screen.getByRole('tab', { name: 'Members' })).toHaveAttribute('aria-selected', 'true');

    rerender(<WorkspaceSettingsModal {...props} viewer={{ role: 'owner', isInstanceAdmin: false }} />);
    expect(screen.getByRole('tab', { name: 'Data retention' })).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByText('retention body')).toBeInTheDocument();
  });
});
