import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { WorkspaceSettingsModal } from './WorkspaceSettingsModal';
import type { Workspace } from '@/stores/workspace';

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

const workspace = { id: 'ws-1', name: 'Dev Team', instanceUrl: 'http://localhost:8080' } as Workspace;

function renderModal(isAdmin: boolean, initial: Parameters<typeof WorkspaceSettingsModal>[0]['initial']) {
  render(
    <WorkspaceSettingsModal
      workspace={workspace}
      isAdmin={isAdmin}
      initial={initial}
      members={[]}
      channels={[]}
      onClose={vi.fn()}
    />,
  );
}

const tabNames = () => screen.getAllByRole('tab').map((t) => t.textContent);

describe('WorkspaceSettingsModal', () => {
  it('gives an admin every section, in one place', () => {
    renderModal(true, 'general');
    expect(tabNames()).toEqual([
      'General',
      'Members',
      'User groups',
      'Custom emoji',
      'Apps',
      'Integrations',
      'Slack import',
      'Audit log',
    ]);
    expect(screen.getByText('general body')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('tab', { name: 'Audit log' }));
    expect(screen.getByText('audit body')).toBeInTheDocument();
  });

  it('gives a member only what they can use', () => {
    renderModal(false, 'members');
    expect(tabNames()).toEqual(['Members', 'User groups', 'Custom emoji', 'Apps']);
  });

  it('falls back to the first section a member may open', () => {
    renderModal(false, 'audit-log');
    expect(screen.getByRole('tab', { name: 'Members' })).toHaveAttribute('aria-selected', 'true');
    expect(screen.queryByText('audit body')).not.toBeInTheDocument();
  });
});
