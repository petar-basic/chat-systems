import { describe, it, expect, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { NewWorkflowDialog } from './NewWorkflowDialog';
import { WorkflowEditor } from './WorkflowEditor';
import type { Starter } from './templates';

const create = vi.fn();

vi.mock('./useWorkflows', () => ({
  useWorkflowActions: () => ({
    create: { isPending: false, mutateAsync: create },
    update: { isPending: false },
    enable: { isPending: false },
    disable: { isPending: false },
    takeOver: { isPending: false },
    remove: { isPending: false },
  }),
}));

vi.mock('@/features/lists/useLists', () => ({
  useWorkspaceLists: () => ({ isLoading: false, data: [] }),
  useListDetail: () => ({ isLoading: false, data: undefined }),
}));

describe('NewWorkflowDialog', () => {
  it('starts from a blank workflow unless a template is picked', () => {
    const onPick = vi.fn<(starter: Starter) => void>();
    render(<NewWorkflowDialog channelId="c-general" onPick={onPick} onClose={vi.fn()} />);
    fireEvent.click(screen.getByTestId('new-workflow-submit'));
    expect(onPick.mock.calls[0][0].name).toBe('');
    expect(onPick.mock.calls[0][0].definition.trigger.type).toBe('schedule');
  });

  it('opens the time-off template with its form, list step and message in the editor', async () => {
    const onPick = vi.fn<(starter: Starter) => void>();
    render(<NewWorkflowDialog channelId="c-general" onPick={onPick} onClose={vi.fn()} />);
    fireEvent.click(screen.getByTestId('new-workflow-template-time_off'));
    fireEvent.click(screen.getByTestId('new-workflow-submit'));
    const starter = onPick.mock.calls[0][0];
    expect(starter.definition.trigger).toEqual({ type: 'shortcut', channel_id: 'c-general' });
    expect(starter.definition.steps.map((step) => step.type)).toEqual([
      'form',
      'add_list_item',
      'send_message',
    ]);

    render(
      <WorkflowEditor
        workspaceId="ws"
        workflow={null}
        starter={starter}
        channels={[]}
        members={[]}
        currentUserId="u-me"
        canBuild
        onClose={vi.fn()}
      />,
    );
    expect(screen.getByTestId('workflow-name')).toHaveValue('Request time off');
    expect(screen.getAllByTestId('workflow-step-title').map((title) => title.textContent)).toEqual([
      'Collect a form',
      'Add a list item',
      'Send a message',
    ]);
    expect(screen.getByTestId('workflow-step-list')).toHaveValue('');
    fireEvent.click(screen.getByTestId('workflow-save'));
    await waitFor(() =>
      expect(screen.getByTestId('workflow-error')).toHaveTextContent('Step 2: pick a list'),
    );
    expect(create).not.toHaveBeenCalled();
  });

  it.each(['welcome', 'standup', 'incident'])('fills the %s template in a channel', (id) => {
    const onPick = vi.fn<(starter: Starter) => void>();
    render(<NewWorkflowDialog channelId="c-general" onPick={onPick} onClose={vi.fn()} />);
    fireEvent.click(screen.getByTestId(`new-workflow-template-${id}`));
    fireEvent.click(screen.getByTestId('new-workflow-submit'));
    const starter = onPick.mock.calls[0][0];
    expect(starter.name).not.toBe('');
    expect(JSON.stringify(starter.definition)).toContain('c-general');
  });
});
