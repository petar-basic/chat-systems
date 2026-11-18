import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { useWorkspaceStore, type Workspace } from '@/stores/workspace';
import { ShortcutMenu } from './ShortcutMenu';

const menu = vi.hoisted(() => ({
  workflows: [] as Array<{ id: string; name: string; description: string | null }>,
  appShortcuts: [] as Array<Record<string, unknown>>,
  start: vi.fn(),
  run: vi.fn(),
  askedFor: [] as Array<string | undefined>,
}));

vi.mock('./useWorkflows', () => ({
  useWorkflowShortcuts: () => ({ data: menu.workflows }),
  useStartWorkflow: () => ({ mutateAsync: menu.start }),
}));
vi.mock('@/features/apps', () => ({
  useGlobalShortcuts: (workspaceId: string | undefined) => {
    menu.askedFor.push(workspaceId);
    return menu.appShortcuts;
  },
  useRunAppShortcut: () => ({ mutate: menu.run }),
}));

const helpdesk = {
  app_id: 'app-1',
  bot_user_id: 'bot-1',
  name: 'Helpdesk',
  description: null,
  icon_url: null,
  interactive: true,
  home: true,
  hook_ids: [],
  shortcuts: [],
};
const newRequest = {
  shortcut: {
    id: 'sc-1',
    app_id: 'app-1',
    kind: 'global',
    name: 'New request',
    description: 'Ask the help desk for something',
    callback_id: 'new_request',
  },
  app: helpdesk,
};

describe('ShortcutMenu', () => {
  beforeEach(() => {
    menu.workflows = [];
    menu.appShortcuts = [];
    menu.askedFor = [];
    menu.start.mockReset();
    menu.run.mockReset();
    useWorkspaceStore.setState({ currentWorkspace: { id: 'ws-1' } as Workspace });
  });

  it('is not there when there is nothing to run', () => {
    render(<ShortcutMenu channelId="ch-1" />);
    expect(screen.queryByTestId('composer-workflows')).not.toBeInTheDocument();
  });

  it('lists the apps’ global shortcuts beside the workflows and sends the channel to the one used', async () => {
    menu.workflows = [{ id: 'wf-1', name: 'Onboard someone', description: null }];
    menu.appShortcuts = [newRequest];
    render(<ShortcutMenu channelId="ch-1" />);

    expect(menu.askedFor).toContain('ws-1');
    fireEvent.click(screen.getByRole('button', { name: 'Shortcuts' }));
    expect(screen.getByRole('menuitem', { name: 'Onboard someone' })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('menuitem', { name: /New request/ }));

    await waitFor(() =>
      expect(menu.run).toHaveBeenCalledWith({ shortcutId: 'sc-1', channelId: 'ch-1' }, expect.anything()),
    );
    expect(menu.start).not.toHaveBeenCalled();
    expect(screen.queryByRole('menu')).not.toBeInTheDocument();
  });

  it('shows an app shortcut even where the channel has no workflows', () => {
    menu.appShortcuts = [newRequest];
    render(<ShortcutMenu channelId="ch-1" />);

    fireEvent.click(screen.getByRole('button', { name: 'Shortcuts' }));
    expect(screen.getAllByRole('menuitem')).toHaveLength(1);
  });
});
