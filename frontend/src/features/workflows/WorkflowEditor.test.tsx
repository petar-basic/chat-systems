import { describe, it, expect, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { Channel, WorkspaceMember } from '@/stores/workspace';
import type { ListColumn } from '@/features/lists/listModel';
import { WorkflowEditor } from './WorkflowEditor';
import type { Definition, OfferedStep, SaveWorkflow, StepProvider, Workflow } from './workflowModel';

const create = vi.fn<(body: SaveWorkflow) => Promise<Workflow>>();
const update = vi.fn<(vars: { id: string; body: SaveWorkflow }) => Promise<Workflow>>();
const enable = vi.fn<(id: string) => Promise<Workflow>>();
const disable = vi.fn<(id: string) => Promise<Workflow>>();
const takeOver = vi.fn<(id: string) => Promise<Workflow>>();
const rotate = vi.fn<(id: string) => Promise<Workflow>>();
const remove = vi.fn<(id: string) => Promise<unknown>>();
const history = vi.hoisted(() => ({ runs: [] as unknown[] }));
const stepApps = vi.hoisted(() => ({ providers: [] as unknown[] }));

const FAILED_RUN = {
  id: 'r-1',
  workflow_version: 1,
  status: 'failed',
  error: 'Step 2 (Send a message): Its creator can no longer post in that channel',
  started_at: '2026-10-24T08:00:00Z',
  finished_at: '2026-10-24T08:00:01Z',
  steps: [
    {
      position: 0,
      label: 'Send a message',
      succeeded: true,
      message_id: 'm-1',
      finished_at: '2026-10-24T08:00:01Z',
    },
    {
      position: 1,
      label: 'Send a message',
      succeeded: false,
      error: 'Its creator can no longer post in that channel',
      finished_at: '2026-10-24T08:00:01Z',
    },
  ],
};

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
  useWorkflowRuns: () => ({ isLoading: false, data: history.runs }),
  useWorkflowStepApps: () => ({ data: stepApps.providers }),
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
  listDetailQuery: (id: string) => ({
    queryKey: ['list', id],
    queryFn: async () => ({ columns: LIST_COLUMNS }),
    initialData: { columns: LIST_COLUMNS },
  }),
}));

vi.mock('@/features/people/hooks/useProfileFields', async (original) => ({
  ...(await original<typeof import('@/features/people/hooks/useProfileFields')>()),
  useProfileFields: () => ({
    fields: [
      {
        id: 'pf-office',
        label: 'Office',
        kind: 'options',
        options: [
          { id: 'o-bg', label: 'Belgrade' },
          { id: 'o-ny', label: 'New York' },
        ],
      },
    ],
  }),
}));

vi.mock('@/features/workspace/hooks/useUserGroups', async (original) => ({
  ...(await original<typeof import('@/features/workspace/hooks/useUserGroups')>()),
  useUserGroups: () => ({ data: [{ id: 'g-design', handle: 'design', name: 'Design' }] }),
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
    <QueryClientProvider client={new QueryClient()}>
      <WorkflowEditor
        workspaceId="ws"
        workflow={workflow}
        channels={CHANNELS}
        members={MEMBERS}
        currentUserId={currentUserId}
        canBuild={canBuild}
        onClose={onClose}
      />
    </QueryClientProvider>,
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

const OPEN_TICKET: OfferedStep = {
  id: 's-1',
  app_id: 'app-1',
  callback_id: 'open_ticket',
  title: 'Open ticket',
  description: 'Files a ticket with the help desk',
  inputs: [
    { name: 'title', title: 'Title', kind: 'text', required: true },
    { name: 'priority', title: 'Priority', kind: 'number', required: false },
    { name: 'reporter', title: 'Reporter', kind: 'person', required: false },
  ],
  outputs: [
    { name: 'ticket_id', title: 'Ticket ID', kind: 'text', required: true },
    { name: 'urgent', title: 'Urgent', kind: 'boolean', required: false },
  ],
};
const TICKETING: StepProvider = {
  app_id: 'app-1',
  name: 'Ticketing',
  icon_url: null,
  problem: null,
  steps: [OPEN_TICKET],
};

beforeEach(() => {
  history.runs = [FAILED_RUN];
  stepApps.providers = [];
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
        webhook_url: 'https://chat.test/api/v1/workflows/webhook/new',
        definition: { trigger: hook, steps: [] },
      }),
    );
    renderEditor(
      saved({
        webhook_url: 'https://chat.test/api/v1/workflows/webhook/old',
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
    const rows = screen.getAllByTestId('workflow-run-step');
    expect(rows).toHaveLength(2);
    expect(rows[0]).toHaveTextContent('Step 1 · Send a message');
    expect(within(rows[1]).getByTestId('workflow-run-step-error')).toHaveTextContent(
      'Its creator can no longer post in that channel',
    );
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

  const lanes = (scope: HTMLElement = document.body) => within(scope).getAllByTestId('workflow-lane');

  it('routes an urgent request one way and any other the other way, each path with its own steps', async () => {
    create.mockResolvedValue(saved());
    renderEditor();
    fireEvent.change(screen.getByTestId('workflow-trigger-type'), { target: { value: 'shortcut' } });
    fireEvent.change(screen.getByTestId('workflow-step-type'), { target: { value: 'form' } });
    fireEvent.change(screen.getByTestId('workflow-step-person'), { target: { value: 'trigger_user' } });
    fireEvent.change(screen.getByTestId('workflow-form-title'), { target: { value: 'Request' } });
    fireEvent.change(screen.getByTestId('workflow-form-field-label'), { target: { value: 'Urgency' } });
    fireEvent.change(screen.getByTestId('workflow-form-field-kind'), { target: { value: 'select' } });
    fireEvent.change(screen.getByTestId('workflow-form-field-options'), {
      target: { value: 'urgent\nnormal' },
    });

    fireEvent.click(screen.getByTestId('workflow-step-add'));
    fireEvent.change(within(steps()[1]).getByTestId('workflow-step-type'), { target: { value: 'branch' } });
    expect(within(steps()[1]).getByTestId('workflow-step-title')).toHaveTextContent('Branch');
    const value = screen.getByTestId('workflow-branch-value');
    expect(optionsOf(value)).toEqual([
      'Pick a value',
      'Person who ran it',
      'Channel',
      'Answer: Urgency',
      'Their profile: Office',
    ]);
    fireEvent.change(value, { target: { value: 'form.urgency' } });
    expect(optionsOf(screen.getByTestId('workflow-branch-op'))).toEqual([
      'is',
      'is one of',
      'contains',
      'is empty',
    ]);
    fireEvent.change(screen.getByTestId('workflow-branch-operand'), { target: { value: 'urgent' } });

    const [then, otherwise] = lanes();
    expect(then).toHaveAccessibleName('Step 2: if it matches');
    expect(within(otherwise).getByText('No steps: the run goes straight on.')).toBeInTheDocument();
    fireEvent.click(within(then).getByTestId('workflow-lane-add'));
    fireEvent.change(within(lanes()[0]).getByTestId('workflow-step-text'), {
      target: { value: 'Paging on-call' },
    });
    fireEvent.click(within(lanes()[1]).getByTestId('workflow-lane-add'));
    fireEvent.change(within(lanes()[1]).getByTestId('workflow-step-text'), { target: { value: 'Filed' } });
    expect(steps().map((step) => step.getAttribute('aria-label'))).toEqual([
      'Step 1',
      'Step 2',
      'Step 3',
      'Step 4',
    ]);
    fireEvent.click(screen.getByTestId('workflow-save'));

    await waitFor(() => expect(create).toHaveBeenCalledTimes(1));
    expect(create.mock.calls[0][0].definition.steps[1]).toEqual({
      type: 'branch',
      value: 'form.urgency',
      test: { op: 'equals', value: 'urgent' },
      then: [{ type: 'send_message', channel_id: 'c-general', text: 'Paging on-call' }],
      otherwise: [{ type: 'send_message', channel_id: 'c-general', text: 'Filed' }],
    });
  });

  it('tests the person who ran it against people picked, or their profile against its choices', () => {
    renderEditor(
      saved({
        definition: {
          trigger: { type: 'shortcut', channel_id: 'c-general' },
          steps: [
            {
              type: 'branch',
              value: 'trigger.user',
              test: { op: 'one_of', values: ['u-bob'] },
              then: [{ type: 'send_message', channel_id: 'c-general', text: 'hi' }],
              otherwise: [],
            },
          ],
        },
      }),
    );
    expect(optionsOf(screen.getByTestId('workflow-branch-op'))).toEqual(['is', 'is one of', 'is empty']);
    expect(screen.getByRole('list', { name: 'People it matches' })).toHaveTextContent('Bob');
    fireEvent.change(screen.getByTestId('workflow-branch-value'), { target: { value: 'profile.pf-office' } });
    expect(screen.queryByRole('list', { name: 'People it matches' })).toBeNull();
    fireEvent.change(screen.getByTestId('workflow-branch-op'), { target: { value: 'equals' } });
    expect(optionsOf(screen.getByTestId('workflow-branch-operand'))).toEqual([
      'Pick a choice',
      'Belgrade',
      'New York',
    ]);
  });

  it('nests branches two deep and no deeper', () => {
    const say = (text: string) => ({ type: 'send_message' as const, channel_id: 'c-general', text });
    const branch = (then: Definition['steps']) => ({
      type: 'branch' as const,
      value: 'trigger.user',
      test: { op: 'is_empty' as const },
      then,
      otherwise: [],
    });
    renderEditor(
      saved({
        definition: {
          trigger: { type: 'shortcut', channel_id: 'c-general' },
          steps: [branch([branch([say('deep')]), say('mid')])],
        },
      }),
    );
    expect(steps().map((step) => step.dataset.type)).toEqual([
      'branch',
      'branch',
      'send_message',
      'send_message',
    ]);
    expect(optionsOf(within(steps()[2]).getByTestId('workflow-step-type'))).not.toContain(
      'Branch on a condition',
    );
    expect(optionsOf(within(steps()[3]).getByTestId('workflow-step-type'))).toContain(
      'Branch on a condition',
    );
    expect(within(steps()[2]).getByTestId('workflow-step-remove')).not.toBeDisabled();
    expect(within(steps()[0]).getByTestId('workflow-step-remove')).toBeDisabled();
  });

  it('won’t save a branch that tests nothing', async () => {
    renderEditor(
      saved({
        definition: {
          trigger: { type: 'shortcut', channel_id: 'c-general' },
          steps: [
            {
              type: 'branch',
              value: '',
              test: { op: 'equals', value: '' },
              then: [{ type: 'send_message', channel_id: 'c-general', text: 'hi' }],
              otherwise: [],
            },
          ],
        },
      }),
    );
    fireEvent.click(screen.getByTestId('workflow-save'));
    expect(await screen.findByTestId('workflow-error')).toHaveTextContent(
      'Step 1: pick what the branch tests',
    );
    expect(update).not.toHaveBeenCalled();
  });

  it('waits a while, and offers the message steps only to a reaction trigger', async () => {
    create.mockResolvedValue(saved());
    renderEditor();
    expect(optionsOf(screen.getByTestId('workflow-step-type'))).not.toContain('React to the message');
    fireEvent.change(screen.getByTestId('workflow-step-type'), { target: { value: 'delay' } });
    expect(screen.getByTestId('workflow-delay-amount')).toHaveValue(1);
    fireEvent.change(screen.getByTestId('workflow-delay-amount'), { target: { value: '2' } });
    fireEvent.change(screen.getByTestId('workflow-delay-unit'), { target: { value: 'hours' } });
    fireEvent.change(screen.getByTestId('workflow-delay-kind'), { target: { value: 'until' } });
    expect(screen.getByTestId('workflow-delay-time')).toHaveValue('09:00');
    fireEvent.change(screen.getByTestId('workflow-delay-kind'), { target: { value: 'for' } });
    fireEvent.change(screen.getByTestId('workflow-delay-amount'), { target: { value: '2' } });
    fireEvent.change(screen.getByTestId('workflow-delay-unit'), { target: { value: 'hours' } });
    fireEvent.click(screen.getByTestId('workflow-step-add'));
    fireEvent.change(screen.getAllByTestId('workflow-step-text')[0], { target: { value: 'Later' } });
    fireEvent.change(screen.getByTestId('workflow-trigger-type'), { target: { value: 'reaction_added' } });
    expect(optionsOf(within(steps()[1]).getByTestId('workflow-step-type'))).toEqual(
      expect.arrayContaining(['React to the message', 'Reply in the message’s thread']),
    );
    fireEvent.click(screen.getByTestId('workflow-trigger-emoji'));
    fireEvent.click(screen.getByText('Siren'));
    fireEvent.click(screen.getByTestId('workflow-save'));

    await waitFor(() => expect(create).toHaveBeenCalledTimes(1));
    expect(create.mock.calls[0][0].definition.steps).toEqual([
      { type: 'delay', wait: { kind: 'for', amount: 2, unit: 'hours' } },
      { type: 'send_message', channel_id: 'c-general', text: 'Later' },
    ]);
  });

  it('makes a project channel, adds the people picked and points later steps at it', async () => {
    create.mockResolvedValue(saved());
    renderEditor();
    fireEvent.change(screen.getByTestId('workflow-trigger-type'), { target: { value: 'shortcut' } });
    fireEvent.change(screen.getByTestId('workflow-step-type'), { target: { value: 'form' } });
    fireEvent.change(screen.getByTestId('workflow-step-person'), { target: { value: 'trigger_user' } });
    fireEvent.change(screen.getByTestId('workflow-form-title'), { target: { value: 'Project' } });
    fireEvent.change(screen.getByTestId('workflow-form-field-label'), { target: { value: 'Name' } });
    fireEvent.click(screen.getByTestId('workflow-form-field-add'));
    fireEvent.change(screen.getAllByTestId('workflow-form-field-label')[1], { target: { value: 'Who' } });
    fireEvent.change(screen.getAllByTestId('workflow-form-field-kind')[1], { target: { value: 'person' } });

    fireEvent.click(screen.getByTestId('workflow-step-add'));
    fireEvent.change(within(steps()[1]).getByTestId('workflow-step-type'), {
      target: { value: 'create_channel' },
    });
    fireEvent.change(screen.getByTestId('workflow-step-channel-name'), {
      target: { value: 'proj-{{form.name}}' },
    });
    fireEvent.click(screen.getByTestId('workflow-step-channel-private'));

    fireEvent.click(screen.getByTestId('workflow-step-add'));
    fireEvent.change(within(steps()[2]).getByTestId('workflow-step-type'), {
      target: { value: 'add_to_channel' },
    });
    const where = within(steps()[2]).getByTestId('workflow-step-channel-ref');
    expect(optionsOf(where)).toEqual([
      'Pick a channel',
      'The channel it started in',
      'The channel step 2 creates',
      '#general',
    ]);
    expect(where).toHaveValue('step:1');
    const add = within(steps()[2]).getByTestId('workflow-step-invitee-add');
    expect(optionsOf(add)).toEqual(['Add people…', 'The person who ran it', 'Answer: Who', '@design']);
    fireEvent.change(add, { target: { value: 'answer:who' } });
    fireEvent.change(within(steps()[2]).getByTestId('workflow-step-invitee-add'), {
      target: { value: 'group:g-design' },
    });
    expect(within(steps()[2]).getByRole('list', { name: 'People to add' })).toHaveTextContent(
      'Answer: Who@design',
    );

    fireEvent.click(screen.getByTestId('workflow-step-add'));
    const reply = steps()[3];
    expect(optionsOf(within(reply).getByTestId('workflow-step-variable'))).toContain(
      'Step 2: link to the new channel',
    );
    fireEvent.change(within(reply).getByTestId('workflow-step-type'), { target: { value: 'send_dm' } });
    fireEvent.change(within(steps()[3]).getByTestId('workflow-step-person'), {
      target: { value: 'trigger_user' },
    });
    fireEvent.change(within(steps()[3]).getByTestId('workflow-step-text'), {
      target: { value: 'Made {{step.2.channel_link}}' },
    });
    fireEvent.click(within(steps()[1]).getByTestId('workflow-step-down'));
    expect(within(steps()[1]).getByTestId('workflow-step-title')).toHaveTextContent(
      'Add people to a channel',
    );
    fireEvent.click(within(steps()[1]).getByTestId('workflow-step-down'));
    expect(within(steps()[1]).getByTestId('workflow-step-title')).toHaveTextContent('Create a channel');
    fireEvent.click(screen.getByTestId('workflow-save'));

    await waitFor(() => expect(create).toHaveBeenCalledTimes(1));
    expect(create.mock.calls[0][0].definition.steps.slice(1)).toEqual([
      { type: 'create_channel', name: 'proj-{{form.name}}', private: true },
      {
        type: 'add_to_channel',
        channel: { kind: 'from_step', step: 1 },
        people: [
          { kind: 'answer', field: 'who' },
          { kind: 'group', group_id: 'g-design' },
        ],
      },
      { type: 'send_dm', to: { kind: 'trigger_user' }, text: 'Made {{step.2.channel_link}}' },
    ]);
  });

  it('runs a step from an app and hands what it gives back to the steps after it', async () => {
    stepApps.providers = [TICKETING];
    create.mockResolvedValue(saved());
    renderEditor();
    fireEvent.change(screen.getByTestId('workflow-trigger-type'), { target: { value: 'shortcut' } });
    const action = screen.getByTestId('workflow-step-type');
    expect(within(action).getByRole('group', { name: 'Ticketing' })).toHaveTextContent('Open ticket');
    fireEvent.change(action, { target: { value: 'app:app-1:open_ticket' } });
    expect(screen.getByTestId('workflow-step-title')).toHaveTextContent('Open ticket');
    expect(screen.getByText(/Files a ticket with the help desk/)).toBeInTheDocument();
    fireEvent.change(screen.getByTestId('workflow-app-input-title'), { target: { value: 'Printer is out' } });
    fireEvent.change(screen.getByTestId('workflow-app-input-priority'), { target: { value: '2' } });
    const reporter = screen.getByTestId('workflow-app-input-reporter');
    expect(optionsOf(reporter)).toEqual(['Leave it empty', 'Person who ran it', 'Me', 'Bob']);
    fireEvent.change(reporter, { target: { value: '{{trigger.user}}' } });
    expect(screen.getByTestId('workflow-app-outputs')).toHaveTextContent(
      'Later steps can use what it hands back: Ticket ID, Urgent.',
    );

    fireEvent.click(screen.getByTestId('workflow-step-add'));
    expect(optionsOf(within(steps()[1]).getByTestId('workflow-step-variable'))).toEqual(
      expect.arrayContaining(['Step 1: Ticket ID', 'Step 1: Urgent']),
    );
    fireEvent.change(within(steps()[1]).getByTestId('workflow-step-text'), {
      target: { value: 'Ticket {{step.1.ticket_id}} opened' },
    });
    fireEvent.click(screen.getByTestId('workflow-save'));

    await waitFor(() => expect(create).toHaveBeenCalledTimes(1));
    expect(create.mock.calls[0][0].definition.steps).toEqual([
      {
        type: 'app_step',
        app_id: 'app-1',
        callback_id: 'open_ticket',
        title: 'Open ticket',
        inputs: OPEN_TICKET.inputs,
        outputs: OPEN_TICKET.outputs,
        values: { title: 'Printer is out', priority: '2', reporter: '{{trigger.user}}' },
      },
      { type: 'send_message', channel_id: 'c-general', text: 'Ticket {{step.1.ticket_id}} opened' },
    ]);
  });

  it('won’t save an app step whose number input holds words', async () => {
    stepApps.providers = [TICKETING];
    renderEditor();
    fireEvent.change(screen.getByTestId('workflow-step-type'), {
      target: { value: 'app:app-1:open_ticket' },
    });
    fireEvent.change(screen.getByTestId('workflow-app-input-title'), { target: { value: 'Printer' } });
    fireEvent.change(screen.getByTestId('workflow-app-input-priority'), { target: { value: 'high' } });
    fireEvent.click(screen.getByTestId('workflow-save'));

    expect(await screen.findByRole('alert')).toHaveTextContent('Step 1: “Priority” needs a number');
    expect(create).not.toHaveBeenCalled();
  });

  it('warns about a step whose app was turned off and shows what each run gave it and got back', () => {
    stepApps.providers = [{ ...TICKETING, problem: 'Ticketing was turned off' }];
    history.runs = [
      {
        id: 'r-3',
        workflow_version: 1,
        status: 'succeeded',
        started_at: '2026-10-24T08:00:00Z',
        finished_at: '2026-10-24T08:00:05Z',
        steps: [
          {
            position: 0,
            label: 'Open ticket',
            succeeded: true,
            inputs: { title: 'Printer' },
            outputs: { ticket_id: 'T-42', urgent: true },
            finished_at: '2026-10-24T08:00:05Z',
          },
        ],
      },
    ];
    renderEditor(
      saved({
        definition: {
          trigger: { type: 'shortcut', channel_id: 'c-general' },
          steps: [
            {
              type: 'app_step',
              app_id: 'app-1',
              callback_id: 'open_ticket',
              title: 'Open ticket',
              inputs: OPEN_TICKET.inputs,
              outputs: OPEN_TICKET.outputs,
              values: { title: 'Printer' },
            },
          ],
        },
      }),
    );
    expect(screen.getByTestId('workflow-app-warning')).toHaveTextContent(
      "Ticketing was turned off. Runs fail here until that's fixed or you replace the step.",
    );
    expect(screen.getByTestId('workflow-step-type')).toHaveValue('app:app-1:open_ticket');
    fireEvent.click(screen.getByTestId('workflow-history-toggle'));
    expect(screen.getByTestId('workflow-run-inputs')).toHaveTextContent('Gave it: title Printer');
    expect(screen.getByTestId('workflow-run-outputs')).toHaveTextContent(
      'Got back: ticket_id T-42, urgent true',
    );
  });

  it('says which path a run took and where it waits, and until when', () => {
    history.runs = [
      {
        id: 'r-2',
        workflow_version: 2,
        status: 'waiting',
        started_at: '2026-10-24T08:00:00Z',
        steps: [
          {
            position: 0,
            label: 'Branch',
            succeeded: true,
            matched: false,
            finished_at: '2026-10-24T08:00:01Z',
          },
          { position: 2, label: 'Send a message', succeeded: true, finished_at: '2026-10-24T08:00:01Z' },
        ],
        waiting: { position: 3, label: 'Wait', until: '2026-10-25T08:00:00Z' },
      },
    ];
    renderEditor(saved());
    fireEvent.click(screen.getByTestId('workflow-history-toggle'));
    expect(screen.getByTestId('workflow-run-status')).toHaveTextContent('Waiting');
    expect(screen.getByTestId('workflow-run-path')).toHaveTextContent('took the “Otherwise” path');
    expect(screen.getAllByTestId('workflow-run-step')[1]).toHaveTextContent('Step 3 · Send a message');
    expect(screen.getByTestId('workflow-run-waiting')).toHaveTextContent(/^Waiting at step 4 · Wait until /);
  });
});
