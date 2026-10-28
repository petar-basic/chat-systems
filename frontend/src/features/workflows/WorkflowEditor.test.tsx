import { describe, it, expect, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import type { Channel, WorkspaceMember } from '@/stores/workspace';
import type { ListColumn } from '@/features/lists/listModel';
import { WorkflowEditor } from './WorkflowEditor';
import type { SaveWorkflow, Workflow } from './workflowModel';

const create = vi.fn<(body: SaveWorkflow) => Promise<Workflow>>();
const update = vi.fn<(vars: { id: string; body: SaveWorkflow }) => Promise<Workflow>>();
const enable = vi.fn<(id: string) => Promise<Workflow>>();
const disable = vi.fn<(id: string) => Promise<Workflow>>();
const takeOver = vi.fn<(id: string) => Promise<Workflow>>();
const rotate = vi.fn<(id: string) => Promise<Workflow>>();
const remove = vi.fn<(id: string) => Promise<unknown>>();

vi.mock('./useWorkflows', () => ({
  useWorkflowActions: () => ({
    create: { mutateAsync: create, isPending: false },
    update: { mutateAsync: update, isPending: false },
    enable: { mutateAsync: enable, isPending: false },
    disable: { mutateAsync: disable, isPending: false },
    takeOver: { mutateAsync: takeOver, isPending: false },
    rotate: { mutateAsync: rotate, isPending: false },
    remove: { mutateAsync: remove, isPending: false },
  }),
  useWorkflowRuns: () => ({
    isLoading: false,
    data: [
      {
        id: 'r-1',
        workflow_version: 1,
        status: 'failed',
        error: 'Step 2 (Send a message): Its creator can no longer post in that channel',
        started_at: '2026-10-24T08:00:00Z',
        finished_at: '2026-10-24T08:00:01Z',
        steps: [
          { position: 0, succeeded: true, message_id: 'm-1', finished_at: '2026-10-24T08:00:01Z' },
          { position: 1, succeeded: false, error: 'no', finished_at: '2026-10-24T08:00:01Z' },
        ],
      },
    ],
  }),
}));

vi.mock('@/features/messaging/EmojiPicker', () => ({
  default: ({ onSelect, onClose }: { onSelect: (emoji: string) => void; onClose: () => void }) => (
    <button
      type="button"
      onClick={() => {
        onSelect('🚨');
        onClose();
      }}
    >
      Siren
    </button>
  ),
}));

vi.mock('@/features/lists/useLists', () => ({
  useWorkspaceLists: () => ({ isLoading: false, data: [{ id: 'l-1', title: 'Time off' }] }),
  useListDetail: (id: string) => ({
    isLoading: false,
    data: id === 'l-1' ? { columns: LIST_COLUMNS } : undefined,
  }),
}));

const column = (id: string, name: string, kind: ListColumn['kind'], choices: string[] = []): ListColumn => ({
  id,
  list_id: 'l-1',
  name,
  kind,
  position: 0,
  hidden: false,
  due: false,
  options: {
    choices: choices.map((label) => ({ id: `${id}-${label.toLowerCase()}`, label, color: 'gray' })),
  },
});
const LIST_COLUMNS = [
  column('request', 'Request', 'text'),
  column('who', 'Who', 'person'),
  column('kind', 'Kind', 'select', ['Sick', 'Holiday']),
  column('from', 'From', 'date'),
  column('days', 'Days', 'number'),
  column('status', 'Status', 'select', ['Open', 'Approved']),
];

const channel = (id: string, name: string, extra: Partial<Channel> = {}) =>
  ({ id, name, channel_type: 'public', is_archived: false, ...extra }) as Channel;
const CHANNELS = [
  channel('c-general', 'general'),
  channel('c-old', 'old', { is_archived: true }),
  channel('c-dm', 'dm', { channel_type: 'dm' }),
];
const member = (user_id: string, display_name: string) =>
  ({ user_id, display_name, email: `${display_name}@x.dev`, role: 'member' }) as WorkspaceMember;
const MEMBERS = [member('u-me', 'Me'), member('u-bob', 'Bob')];

const saved = (overrides: Partial<Workflow> = {}): Workflow => ({
  id: 'w-1',
  workspace_id: 'ws',
  name: 'Standup',
  description: null,
  created_by: 'u-me',
  version: 1,
  enabled: false,
  creator_gone: false,
  webhook_url: null,
  next_run_at: null,
  last_run: null,
  created_at: '2026-10-24T08:00:00Z',
  updated_at: '2026-10-24T08:00:00Z',
  definition: {
    trigger: { type: 'schedule', days: ['mon', 'tue', 'wed', 'thu', 'fri'], time: '09:50', timezone: 'UTC' },
    steps: [
      { type: 'send_message', channel_id: 'c-general', text: 'one' },
      { type: 'send_message', channel_id: 'c-general', text: 'two' },
    ],
  },
  ...overrides,
});

function renderEditor(workflow: Workflow | null = null, { currentUserId = 'u-me', canBuild = true } = {}) {
  const onClose = vi.fn();
  render(
    <WorkflowEditor
      workspaceId="ws"
      workflow={workflow}
      channels={CHANNELS}
      members={MEMBERS}
      currentUserId={currentUserId}
      canBuild={canBuild}
      onClose={onClose}
    />,
  );
  return onClose;
}

const steps = () => screen.getAllByTestId('workflow-step');
const cell = (scope: HTMLElement, name: string) =>
  within(scope)
    .getAllByTestId('workflow-cell')
    .find((row) => row.dataset.column === name) as HTMLElement;
const optionsOf = (select: HTMLElement) =>
  within(select)
    .getAllByRole('option')
    .map((o) => o.textContent);
const texts = () =>
  screen.getAllByTestId('workflow-step-text').map((area) => (area as HTMLTextAreaElement).value);

beforeEach(() => {
  create.mockReset();
  update.mockReset();
  enable.mockReset();
  disable.mockReset();
  rotate.mockReset();
  remove.mockReset();
});

describe('WorkflowEditor', () => {
  it('starts a new workflow with a weekday schedule posting to the first open channel', () => {
    renderEditor();
    expect(screen.getByTestId('workflow-trigger-type')).toHaveValue('schedule');
    expect(screen.getByTestId('workflow-schedule-repeat')).toHaveValue('weekdays');
    expect(steps()).toHaveLength(1);
    const options = within(screen.getByTestId('workflow-step-channel'))
      .getAllByRole('option')
      .map((o) => o.textContent);
    expect(options).toEqual(['Pick a channel', '#general']);
    expect(screen.getByTestId('workflow-step-channel')).toHaveValue('c-general');
    expect(screen.queryByTestId('workflow-delete')).toBeNull();
  });

  it('adds, reorders and removes steps', () => {
    renderEditor(saved());
    fireEvent.click(screen.getByTestId('workflow-step-add'));
    expect(steps()).toHaveLength(3);
    fireEvent.change(screen.getAllByTestId('workflow-step-text')[2], { target: { value: 'three' } });

    fireEvent.click(within(steps()[2]).getByTestId('workflow-step-up'));
    expect(texts()).toEqual(['one', 'three', 'two']);
    expect(within(steps()[0]).getByTestId('workflow-step-up')).toBeDisabled();
    expect(within(steps()[2]).getByTestId('workflow-step-down')).toBeDisabled();

    fireEvent.click(within(steps()[0]).getByTestId('workflow-step-remove'));
    expect(texts()).toEqual(['three', 'two']);
  });

  it('switches a step to a DM to someone other than the builder', () => {
    renderEditor();
    fireEvent.change(screen.getByTestId('workflow-step-type'), { target: { value: 'send_dm' } });
    expect(screen.getByTestId('workflow-step-title')).toHaveTextContent('Send a DM');
    const people = within(screen.getByTestId('workflow-step-person'))
      .getAllByRole('option')
      .map((o) => o.textContent);
    expect(people).toEqual(['Pick a person', 'Bob']);
  });

  it('declares webhook variables and inserts them where the caret is', () => {
    renderEditor();
    fireEvent.change(screen.getByTestId('workflow-trigger-type'), { target: { value: 'webhook' } });
    expect(screen.getByText('Save the workflow to get its URL.')).toBeInTheDocument();
    expect(screen.queryByTestId('workflow-step-variable')).toBeNull();

    fireEvent.click(screen.getByTestId('workflow-variable-add'));
    fireEvent.change(screen.getByTestId('workflow-variable-name'), { target: { value: 'service' } });

    const area = screen.getByTestId('workflow-step-text') as HTMLTextAreaElement;
    fireEvent.change(area, { target: { value: 'is down' } });
    area.setSelectionRange(0, 0);
    fireEvent.change(screen.getByTestId('workflow-step-variable'), {
      target: { value: '{{webhook.service}}' },
    });
    expect(area.value).toBe('{{webhook.service}}is down');
  });

  it('builds a welcome for people who join a channel, sent back to them', async () => {
    create.mockResolvedValue(saved());
    renderEditor();
    fireEvent.change(screen.getByTestId('workflow-trigger-type'), { target: { value: 'member_joined' } });
    expect(screen.getByTestId('workflow-trigger-channel')).toHaveValue('c-general');
    expect(
      within(screen.getByTestId('workflow-step-variable'))
        .getAllByRole('option')
        .map((o) => o.textContent),
    ).toEqual(['Insert variable…', 'Person who joined', 'Channel']);

    fireEvent.change(screen.getByTestId('workflow-step-type'), { target: { value: 'send_dm' } });
    fireEvent.change(screen.getByTestId('workflow-step-person'), { target: { value: 'trigger_user' } });
    fireEvent.change(screen.getByTestId('workflow-step-text'), { target: { value: 'Welcome!' } });
    fireEvent.click(screen.getByTestId('workflow-save'));

    await waitFor(() => expect(create).toHaveBeenCalledTimes(1));
    expect(create.mock.calls[0][0].definition).toEqual({
      trigger: { type: 'member_joined', channel_id: 'c-general' },
      steps: [{ type: 'send_dm', to: { kind: 'trigger_user' }, text: 'Welcome!' }],
    });
  });

  it('builds a shortcut that asks the person who ran it a form, then posts the answers', async () => {
    create.mockResolvedValue(saved());
    renderEditor();
    fireEvent.change(screen.getByTestId('workflow-trigger-type'), { target: { value: 'shortcut' } });
    expect(screen.getByTestId('workflow-trigger-channel')).toHaveValue('c-general');

    fireEvent.change(screen.getByTestId('workflow-step-type'), { target: { value: 'form' } });
    expect(screen.getByTestId('workflow-step-title')).toHaveTextContent('Collect a form');
    fireEvent.change(screen.getByTestId('workflow-step-person'), { target: { value: 'trigger_user' } });
    fireEvent.change(screen.getByTestId('workflow-form-title'), { target: { value: 'Time off' } });
    fireEvent.change(screen.getByTestId('workflow-form-field-label'), { target: { value: 'How long?' } });
    expect(screen.getByTestId('workflow-form-field-variable')).toHaveTextContent('{{form.how_long}}');
    fireEvent.click(screen.getByTestId('workflow-form-field-add'));
    fireEvent.change(screen.getAllByTestId('workflow-form-field-label')[1], { target: { value: 'Kind' } });
    fireEvent.change(screen.getAllByTestId('workflow-form-field-kind')[1], { target: { value: 'select' } });
    fireEvent.change(screen.getByTestId('workflow-form-field-options'), {
      target: { value: 'Sick\n\nHoliday\n' },
    });

    fireEvent.click(screen.getByTestId('workflow-step-add'));
    const next = steps()[1];
    expect(
      within(within(next).getByTestId('workflow-step-variable'))
        .getAllByRole('option')
        .map((o) => o.textContent),
    ).toEqual(['Insert variable…', 'Person who ran it', 'Channel', 'Answer: How long?', 'Answer: Kind']);
    fireEvent.change(within(next).getByTestId('workflow-step-text'), { target: { value: 'New request' } });
    fireEvent.click(within(next).getByTestId('workflow-step-summary'));
    fireEvent.click(screen.getByTestId('workflow-save'));

    await waitFor(() => expect(create).toHaveBeenCalledTimes(1));
    expect(create.mock.calls[0][0].definition).toEqual({
      trigger: { type: 'shortcut', channel_id: 'c-general' },
      steps: [
        {
          type: 'form',
          to: { kind: 'trigger_user' },
          title: 'Time off',
          fields: [
            { name: 'how_long', label: 'How long?', kind: 'short_text', options: [], required: true },
            { name: 'kind', label: 'Kind', kind: 'select', options: ['Sick', 'Holiday'], required: true },
          ],
        },
        { type: 'send_message', channel_id: 'c-general', text: 'New request', summary: true },
      ],
    });
  });

  it('adds the form answers to a list, then updates the status of that item', async () => {
    update.mockResolvedValue(saved());
    renderEditor(
      saved({
        definition: {
          trigger: { type: 'shortcut', channel_id: 'c-general' },
          steps: [
            {
              type: 'form',
              to: { kind: 'trigger_user' },
              title: 'Time off',
              fields: [
                { name: 'kind', label: 'Kind', kind: 'select', options: ['Sick', 'Holiday'], required: true },
                { name: 'from', label: 'From', kind: 'date', options: [], required: true },
                { name: 'note', label: 'Note', kind: 'short_text', options: [], required: false },
              ],
            },
          ],
        },
      }),
    );

    expect(optionsOf(screen.getByTestId('workflow-step-type'))).not.toContain('Update the item a step added');
    fireEvent.click(screen.getByTestId('workflow-step-add'));
    fireEvent.change(within(steps()[1]).getByTestId('workflow-step-type'), {
      target: { value: 'add_list_item' },
    });
    expect(within(steps()[1]).getByTestId('workflow-step-title')).toHaveTextContent('Add a list item');
    fireEvent.change(within(steps()[1]).getByTestId('workflow-step-list'), { target: { value: 'l-1' } });
    fireEvent.click(within(steps()[1]).getByTestId('workflow-step-match'));
    expect(within(cell(steps()[1], 'Kind')).getByTestId('workflow-cell-value')).toHaveValue('{{form.kind}}');
    expect(within(cell(steps()[1], 'From')).getByTestId('workflow-cell-value')).toHaveValue('{{form.from}}');

    const who = within(cell(steps()[1], 'Who')).getByTestId('workflow-cell-value');
    expect(optionsOf(who)).toContain('Person who ran it');
    expect(optionsOf(who)).not.toContain('Answer: From');
    fireEvent.change(who, { target: { value: '{{trigger.user}}' } });
    fireEvent.change(within(cell(steps()[1], 'Request')).getByTestId('workflow-cell-variable'), {
      target: { value: '{{form.note}}' },
    });

    fireEvent.click(screen.getByTestId('workflow-step-add'));
    fireEvent.change(within(steps()[2]).getByTestId('workflow-step-type'), {
      target: { value: 'update_list_item' },
    });
    expect(within(steps()[2]).getByTestId('workflow-step-from')).toHaveValue('1');
    expect(
      within(steps()[2])
        .getAllByTestId('workflow-cell')
        .map((row) => row.dataset.column),
    ).toEqual(['Who', 'Kind', 'Status']);
    fireEvent.change(within(cell(steps()[2], 'Status')).getByTestId('workflow-cell-value'), {
      target: { value: 'status-approved' },
    });
    fireEvent.click(screen.getByTestId('workflow-save'));

    await waitFor(() => expect(update).toHaveBeenCalledTimes(1));
    expect(update.mock.calls[0][0].body.definition.steps.slice(1)).toEqual([
      {
        type: 'add_list_item',
        list_id: 'l-1',
        cells: [
          { column_id: 'kind', value: '{{form.kind}}' },
          { column_id: 'from', value: '{{form.from}}' },
          { column_id: 'who', value: '{{trigger.user}}' },
          { column_id: 'request', value: '{{form.note}}' },
        ],
      },
      { type: 'update_list_item', from_step: 1, cells: [{ column_id: 'status', value: 'status-approved' }] },
    ]);
  });

  it('asks an update step for a new item once the step that added its item is removed', () => {
    renderEditor(
      saved({
        definition: {
          trigger: { type: 'shortcut', channel_id: 'c-general' },
          steps: [
            { type: 'send_message', channel_id: 'c-general', text: 'one' },
            { type: 'add_list_item', list_id: 'l-1', cells: [{ column_id: 'request', value: 'x' }] },
            {
              type: 'update_list_item',
              from_step: 1,
              cells: [{ column_id: 'status', value: 'status-open' }],
            },
          ],
        },
      }),
    );
    fireEvent.click(within(steps()[0]).getByTestId('workflow-step-down'));
    expect(within(steps()[2]).getByTestId('workflow-step-from')).toHaveValue('0');
    fireEvent.click(within(steps()[0]).getByTestId('workflow-step-remove'));
    expect(within(steps()[1]).getByTestId('workflow-step-from')).toHaveValue('');
    expect(within(steps()[1]).queryByTestId('workflow-cell')).toBeNull();
  });

  it('picks the emoji a reaction trigger listens for', () => {
    renderEditor();
    fireEvent.change(screen.getByTestId('workflow-trigger-type'), { target: { value: 'reaction_added' } });
    expect(screen.getByTestId('workflow-trigger-emoji')).toHaveTextContent('Pick an emoji');
    fireEvent.click(screen.getByTestId('workflow-trigger-emoji'));
    fireEvent.click(screen.getByRole('button', { name: 'Siren' }));
    expect(screen.getByTestId('workflow-trigger-emoji')).toHaveTextContent('🚨');
    expect(screen.queryByRole('button', { name: 'Siren' })).toBeNull();
  });

  it('offers no trigger person to a schedule’s DM', () => {
    renderEditor();
    fireEvent.change(screen.getByTestId('workflow-step-type'), { target: { value: 'send_dm' } });
    expect(
      within(screen.getByTestId('workflow-step-person'))
        .getAllByRole('option')
        .map((o) => (o as HTMLOptionElement).value),
    ).not.toContain('trigger_user');
  });

  it('flags a variable name the server would refuse', () => {
    renderEditor();
    fireEvent.change(screen.getByTestId('workflow-trigger-type'), { target: { value: 'webhook' } });
    fireEvent.click(screen.getByTestId('workflow-variable-add'));
    fireEvent.change(screen.getByTestId('workflow-variable-name'), { target: { value: 'Service' } });
    expect(screen.getByText('Lowercase letters, digits and _, starting with a letter')).toBeInTheDocument();
  });

  it('creates a workflow that runs at once, then saves changes and turns it off and on', async () => {
    create.mockResolvedValue(saved({ enabled: true }));
    update.mockResolvedValue(saved({ version: 2, enabled: true }));
    disable.mockResolvedValue(saved({ version: 2, enabled: false }));
    enable.mockResolvedValue(saved({ version: 2, enabled: true }));
    renderEditor();
    expect(screen.getByTestId('workflow-save')).toHaveTextContent('Create workflow');
    fireEvent.change(screen.getByTestId('workflow-name'), { target: { value: '  Standup ' } });
    fireEvent.change(screen.getByTestId('workflow-step-text'), { target: { value: 'standup in 10' } });
    fireEvent.click(screen.getByTestId('workflow-save'));

    await waitFor(() => expect(create).toHaveBeenCalledTimes(1));
    expect(create.mock.calls[0][0]).toEqual({
      name: 'Standup',
      description: null,
      definition: {
        trigger: expect.objectContaining({ type: 'schedule', days: ['mon', 'tue', 'wed', 'thu', 'fri'] }),
        steps: [{ type: 'send_message', channel_id: 'c-general', text: 'standup in 10' }],
      },
    });
    expect(await screen.findByTestId('workflow-disable')).toBeInTheDocument();
    expect(screen.getByTestId('workflow-save')).toHaveTextContent('Save changes');

    fireEvent.click(screen.getByTestId('workflow-save'));
    await waitFor(() => expect(update).toHaveBeenCalledWith(expect.objectContaining({ id: 'w-1' })));
    fireEvent.click(screen.getByTestId('workflow-disable'));
    await waitFor(() => expect(disable).toHaveBeenCalledWith('w-1'));
    fireEvent.click(await screen.findByTestId('workflow-enable'));
    await waitFor(() => expect(enable).toHaveBeenCalledWith('w-1'));
    expect(await screen.findByTestId('workflow-disable')).toBeInTheDocument();
  });

  it('drops blank webhook variables before saving', async () => {
    create.mockResolvedValue(saved());
    renderEditor();
    fireEvent.change(screen.getByTestId('workflow-trigger-type'), { target: { value: 'webhook' } });
    fireEvent.click(screen.getByTestId('workflow-variable-add'));
    fireEvent.click(screen.getByTestId('workflow-variable-add'));
    fireEvent.change(screen.getAllByTestId('workflow-variable-name')[1], { target: { value: 'code' } });
    fireEvent.click(screen.getByTestId('workflow-save'));
    await waitFor(() => expect(create).toHaveBeenCalled());
    expect(create.mock.calls[0][0].definition.trigger).toEqual({ type: 'webhook', variables: ['code'] });
  });

  it('shows why the server refused a save', async () => {
    create.mockRejectedValue(new Error('Step 1: you can’t post in that channel'));
    renderEditor();
    fireEvent.click(screen.getByTestId('workflow-save'));
    expect(await screen.findByTestId('workflow-error')).toHaveTextContent(
      'Step 1: you can’t post in that channel',
    );
  });

  it('lets an admin read and turn off someone else’s workflow, not edit it', async () => {
    disable.mockResolvedValue(saved({ created_by: 'u-bob', enabled: false }));
    renderEditor(saved({ created_by: 'u-bob', enabled: true }), { currentUserId: 'u-me' });
    expect(screen.getByTestId('workflow-name')).toBeDisabled();
    expect(screen.queryByTestId('workflow-save')).toBeNull();
    expect(screen.queryByTestId('workflow-enable')).toBeNull();
    expect(screen.queryByTestId('workflow-step-add')).toBeNull();
    expect(screen.getByTestId('workflow-read-only')).toHaveTextContent(
      'Only the person who built this workflow',
    );
    expect(screen.queryByTestId('workflow-take-over')).toBeNull();
    fireEvent.click(screen.getByTestId('workflow-disable'));
    await waitFor(() => expect(disable).toHaveBeenCalledWith('w-1'));
  });

  it('lets an admin take over a workflow whose builder is gone, then edit it', async () => {
    takeOver.mockResolvedValue(saved({ created_by: 'u-me' }));
    renderEditor(saved({ created_by: 'u-bob', creator_gone: true }), { currentUserId: 'u-me' });
    expect(screen.getByTestId('workflow-read-only')).toHaveTextContent('left or was suspended');
    expect(screen.getByTestId('workflow-name')).toBeDisabled();
    fireEvent.click(screen.getByTestId('workflow-take-over'));
    await waitFor(() => expect(takeOver).toHaveBeenCalledWith('w-1'));
    await waitFor(() => expect(screen.getByTestId('workflow-name')).not.toBeDisabled());
    expect(screen.getByTestId('workflow-enable')).toBeInTheDocument();
    expect(screen.queryByTestId('workflow-take-over')).toBeNull();
  });

  it('replaces a webhook workflow’s URL and shows the new one', async () => {
    const hook = { type: 'webhook' as const, variables: [] };
    rotate.mockResolvedValue(
      saved({
        webhook_url: 'https://chat.test/api/workflows/webhook/new',
        definition: { trigger: hook, steps: [] },
      }),
    );
    renderEditor(
      saved({
        webhook_url: 'https://chat.test/api/workflows/webhook/old',
        definition: { trigger: hook, steps: [] },
      }),
    );
    expect(screen.getByTestId('workflow-webhook-url')).toHaveTextContent('/webhook/old');
    fireEvent.click(screen.getByTestId('workflow-rotate'));
    await waitFor(() => expect(rotate).toHaveBeenCalledWith('w-1'));
    await waitFor(() => expect(screen.getByTestId('workflow-webhook-url')).toHaveTextContent('/webhook/new'));
  });

  it('shows the run history with each step’s outcome', () => {
    renderEditor(saved());
    fireEvent.click(screen.getByTestId('workflow-history-toggle'));
    expect(screen.getByTestId('workflow-run-status')).toHaveTextContent('Failed');
    expect(screen.getByTestId('workflow-run-error')).toHaveTextContent('Its creator can no longer post');
    expect(screen.getAllByTestId('workflow-run-step').map((s) => s.textContent)).toEqual([
      'Step 1 · Send a message',
      'Step 2 · Send a message',
    ]);
  });

  it('deletes after confirming and closes', async () => {
    remove.mockResolvedValue({});
    const onClose = renderEditor(saved());
    fireEvent.click(screen.getByTestId('workflow-delete'));
    expect(remove).not.toHaveBeenCalled();
    fireEvent.click(screen.getByTestId('workflow-delete-confirm-confirm'));
    await waitFor(() => expect(remove).toHaveBeenCalledWith('w-1'));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
  });
});
