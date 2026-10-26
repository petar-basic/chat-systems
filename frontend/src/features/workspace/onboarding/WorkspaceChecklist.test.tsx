import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { WorkspaceChecklist } from './WorkspaceChecklist';
import type { Channel, Workspace, WorkspaceMember } from '@/stores/workspace';

let invites: unknown[] = [];
let ssoEnabled = false;

vi.mock('@/features/workspace/hooks/useWorkspaces', () => ({
  useWorkspaceInvites: () => ({ data: invites }),
}));

vi.mock('@/features/instance/hooks/useInstanceInfo', () => ({
  useInstanceInfo: () => ({ data: { sso_enabled: ssoEnabled } }),
}));

const workspace = (over: Partial<Workspace> = {}) =>
  ({ id: 'ws-1', name: 'Acme', icon_url: null, instanceUrl: 'http://chat.test', ...over }) as Workspace;

const member = (id: string) => ({ user_id: id }) as WorkspaceMember;

const channel = (name: string, over: Partial<Channel> = {}) =>
  ({ id: name, name, channel_type: 'public', is_default: false, ...over }) as Channel;

const general = channel('general', { is_default: true });

function renderChecklist(props: {
  workspace?: Workspace;
  members?: WorkspaceMember[];
  channels?: Channel[];
}) {
  const handlers = { onInvite: vi.fn(), onCreateChannel: vi.fn(), onSetIcon: vi.fn() };
  const view = render(
    <WorkspaceChecklist
      workspace={props.workspace ?? workspace()}
      members={props.members ?? [member('me')]}
      channels={props.channels ?? [general]}
      {...handlers}
    />,
  );
  return { ...view, handlers };
}

function doneSteps() {
  return screen
    .getAllByTestId('checklist-step')
    .filter((li) => li.dataset.done === 'true')
    .map((li) => li.dataset.step);
}

describe('WorkspaceChecklist', () => {
  beforeEach(() => {
    invites = [];
    ssoEnabled = false;
    localStorage.clear();
  });

  it('starts with nothing done and offers each step', () => {
    const { handlers } = renderChecklist({});

    expect(screen.getByTestId('checklist-progress')).toHaveTextContent('0 of 4 done');
    fireEvent.click(screen.getByTestId('checklist-invite'));
    fireEvent.click(screen.getByTestId('checklist-channel'));
    fireEvent.click(screen.getByTestId('checklist-icon'));
    expect(handlers.onInvite).toHaveBeenCalled();
    expect(handlers.onCreateChannel).toHaveBeenCalled();
    expect(handlers.onSetIcon).toHaveBeenCalled();
    expect(screen.getByTestId('checklist-sso')).toHaveAttribute(
      'href',
      expect.stringContaining('RUNBOOK.md'),
    );
  });

  it('ticks a step off once it is done', () => {
    const { rerender, handlers } = renderChecklist({});
    expect(doneSteps()).toEqual([]);

    invites = [{ id: 'inv-1' }];
    rerender(
      <WorkspaceChecklist
        workspace={workspace({ icon_url: 'https://acme.test/icon.png' })}
        members={[member('me')]}
        channels={[general]}
        {...handlers}
      />,
    );
    expect(doneSteps()).toEqual(['invite', 'icon']);
    expect(screen.queryByTestId('checklist-invite')).toBeNull();

    ssoEnabled = true;
    rerender(
      <WorkspaceChecklist
        workspace={workspace({ icon_url: 'https://acme.test/icon.png' })}
        members={[member('me'), member('bob')]}
        channels={[general, channel('design', { channel_type: 'private' })]}
        {...handlers}
      />,
    );
    expect(screen.queryByTestId('workspace-checklist')).toBeNull();
  });

  it('a direct message is not a channel of your own', () => {
    renderChecklist({ channels: [general, channel('dm', { channel_type: 'dm' })] });
    expect(doneSteps()).not.toContain('channel');
  });

  it('stays away from a workspace that is already going', () => {
    renderChecklist({
      members: [member('me'), member('bob'), member('ada')],
      channels: [general, channel('design')],
    });
    expect(screen.queryByTestId('workspace-checklist')).toBeNull();
  });

  it('is dismissed for good', () => {
    const { unmount } = renderChecklist({});
    fireEvent.click(screen.getByTestId('checklist-dismiss'));
    expect(screen.queryByTestId('workspace-checklist')).toBeNull();

    unmount();
    renderChecklist({});
    expect(screen.queryByTestId('workspace-checklist')).toBeNull();

    renderChecklist({ workspace: workspace({ id: 'ws-2' }) });
    expect(screen.getByTestId('workspace-checklist')).toBeInTheDocument();
  });
});
