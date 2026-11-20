import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import i18n from '@/lib/i18n';
import { WorkspaceChecklist } from './WorkspaceChecklist';
import type { Channel, Workspace, WorkspaceMember } from '@/stores/workspace';

const hostile = '<img src=x onerror="alert(1)"> {{name}}';

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

function stepTexts() {
  return screen.getAllByTestId('checklist-step').map((li) => li.textContent);
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

  it('folds its steps away and back', () => {
    renderChecklist({});
    const toggle = screen.getByTestId('checklist-toggle');
    expect(screen.getByTestId('checklist-invite')).toBeVisible();

    fireEvent.click(toggle);
    expect(toggle).toHaveAttribute('aria-expanded', 'false');
    expect(screen.getByTestId('checklist-invite')).not.toBeVisible();

    fireEvent.click(toggle);
    expect(screen.getByTestId('checklist-invite')).toBeVisible();
  });

  it('starts folded on a phone, keeping the progress in view', () => {
    vi.stubGlobal('matchMedia', () => ({ matches: true }));
    try {
      renderChecklist({});
      expect(screen.getByTestId('checklist-progress')).toHaveTextContent('0 of 4 done');
      expect(screen.getByTestId('checklist-toggle')).toHaveTextContent('Show steps');
      expect(screen.getByTestId('checklist-invite')).not.toBeVisible();
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it('names the workspace and each step, and marks the ones done', () => {
    invites = [{ id: 'inv-1' }];
    renderChecklist({});
    expect(screen.getByRole('heading', { name: 'Get Acme going' })).toBeInTheDocument();
    expect(screen.getByTestId('checklist-progress')).toHaveTextContent('1 of 4 done');
    expect(screen.getByTestId('checklist-toggle')).toHaveTextContent('Hide steps');
    expect(screen.getByRole('button', { name: 'Dismiss the checklist' })).toBeInTheDocument();
    expect(stepTexts()).toEqual([
      'Invite your team (done)',
      'Create a channelCreate',
      'Set an iconChoose',
      'Connect SSOHow to ',
    ]);
  });

  it('shows the workspace name as text', () => {
    renderChecklist({ workspace: workspace({ name: hostile }) });
    expect(screen.getByRole('heading').textContent).toBe(`Get ${hostile} going`);
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  describe('in Japanese', () => {
    beforeEach(async () => {
      await i18n.changeLanguage('ja');
    });

    afterEach(async () => {
      await i18n.changeLanguage('en');
    });

    it('names the workspace and each step, and marks the ones done', () => {
      invites = [{ id: 'inv-1' }];
      renderChecklist({});
      expect(screen.getByRole('heading', { name: 'Acme の準備を進めましょう' })).toBeInTheDocument();
      expect(screen.getByTestId('checklist-progress')).toHaveTextContent('4 件中 1 件完了');
      expect(screen.getByRole('button', { name: 'チェックリストを閉じる' })).toBeInTheDocument();
      expect(stepTexts()).toEqual([
        'チームのメンバーを招待する（完了）',
        'チャンネルを作成する作成',
        'アイコンを設定する選択',
        'SSO（シングルサインオン）を設定する手順 ',
      ]);

      const toggle = screen.getByTestId('checklist-toggle');
      expect(toggle).toHaveTextContent('手順を隠す');
      fireEvent.click(toggle);
      expect(toggle).toHaveTextContent('手順を表示');
    });

    it('offers each step', () => {
      renderChecklist({});
      expect(screen.getByRole('button', { name: 'メンバーを招待' })).toBeInTheDocument();
      expect(screen.getByRole('button', { name: '作成' })).toBeInTheDocument();
      expect(screen.getByRole('button', { name: '選択' })).toBeInTheDocument();
      expect(screen.getByRole('link', { name: '手順' })).toHaveAttribute(
        'href',
        expect.stringContaining('RUNBOOK.md'),
      );
    });

    it('shows the workspace name as text', () => {
      renderChecklist({ workspace: workspace({ name: hostile }) });
      expect(screen.getByRole('heading').textContent).toBe(`${hostile} の準備を進めましょう`);
      expect(document.querySelector('[onerror]')).toBeNull();
    });

    it('has Japanese for every one', () => {
      function leaves(value: unknown, prefix = ''): string[] {
        if (typeof value !== 'object' || value === null) return [prefix];
        return Object.entries(value).flatMap(([key, child]) =>
          leaves(child, prefix ? `${prefix}.${key}` : key),
        );
      }
      const english = leaves(i18n.getResource('en', 'workspace', 'checklist'));
      const japanese = leaves(i18n.getResource('ja', 'workspace', 'checklist'));
      expect(english.length).toBeGreaterThan(0);
      expect(japanese).toEqual(english);
    });
  });
});
