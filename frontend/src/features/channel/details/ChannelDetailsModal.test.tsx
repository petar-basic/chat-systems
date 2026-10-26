import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor, within } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ChannelDetailsModal, type ChannelDetailsTab } from './ChannelDetailsModal';
import type { Channel } from '@/stores/workspace';
import type { ChannelRetention } from '@/features/workspace/hooks/useRetention';
import type { ChannelIntegrations } from '../hooks/useChannelIntegrations';

const updateChannel = vi.fn();
const setRetention = vi.fn();
const createHook = vi.fn();
const revealHook = vi.fn();
const rotateHook = vi.fn();
const deleteHook = vi.fn();
let retention: ChannelRetention;
let integrations: ChannelIntegrations;

vi.mock('../hooks/useChannels', () => ({
  useUpdateChannel: () => ({ mutateAsync: updateChannel, isPending: false }),
  useArchiveChannel: () => ({ mutateAsync: vi.fn(), isPending: false }),
}));
vi.mock('@/features/workspace/hooks/useRetention', () => ({
  useChannelRetention: () => ({ data: retention }),
  useSetChannelRetention: () => ({ mutateAsync: setRetention, isPending: false }),
}));
vi.mock('@/features/workspace/hooks/useHooks', () => ({
  useCreateHook: () => ({ mutateAsync: createHook, isPending: false }),
  useRevealHook: () => ({ mutateAsync: revealHook, isPending: false }),
  useRotateHook: () => ({ mutateAsync: rotateHook, isPending: false }),
  useDeleteHook: () => ({ mutateAsync: deleteHook, isPending: false }),
  useHookedChannels: () => ({ data: new Set() }),
}));
vi.mock('@/features/workspace/hooks/useWorkspaces', () => ({
  useWorkspaceMembers: () => ({ data: [{ user_id: 'u-ana', display_name: 'Ana' }] }),
  useWorkspaceChannels: () => ({ data: [] }),
}));
vi.mock('../hooks/useChannelIntegrations', () => ({
  useChannelIntegrations: () => ({ isLoading: false, data: integrations }),
}));

const channel = {
  id: 'ch-1',
  workspace_id: 'ws-1',
  name: 'deploys',
  topic: 'Shipping',
  description: '',
  settings: {},
} as unknown as Channel;

function renderModal({
  canModerate = true,
  initialTab = 'about',
  onOpenWorkflow,
}: { canModerate?: boolean; initialTab?: ChannelDetailsTab; onOpenWorkflow?: (id: string) => void } = {}) {
  const onClose = vi.fn();
  render(
    <QueryClientProvider client={new QueryClient()}>
      <ChannelDetailsModal
        channel={channel}
        canModerate={canModerate}
        initialTab={initialTab}
        onOpenWorkflow={onOpenWorkflow}
        onClose={onClose}
        onArchived={vi.fn()}
      />
    </QueryClientProvider>,
  );
  return onClose;
}

const tabs = () => screen.getAllByRole('tab').map((tab) => tab.textContent);
const save = () => fireEvent.click(screen.getByTestId('channel-settings-save'));

beforeEach(() => {
  updateChannel.mockReset().mockResolvedValue({});
  setRetention.mockReset().mockResolvedValue({});
  createHook.mockReset().mockResolvedValue({ id: 'h-new' });
  revealHook.mockReset().mockResolvedValue({ incoming_url: 'https://chat.test/api/hooks/incoming/first' });
  rotateHook.mockReset().mockResolvedValue({ incoming_url: 'https://chat.test/api/hooks/incoming/second' });
  deleteHook.mockReset().mockResolvedValue({});
  retention = { overrides_allowed: true, workspace_message_days: 365, channel: null };
  integrations = {
    incoming_webhooks: [
      {
        id: 'h-ci',
        name: 'CI',
        description: null,
        created_by: 'u-ana',
        created_at: '2026-10-20T10:00:00Z',
      },
    ],
    outgoing_webhooks: [{ id: 'h-out', name: 'Mirror', description: 'to the archive' }],
    slash_commands: [{ id: 'h-cmd', name: 'Deployer', command: 'deploy' }],
    workflows: [{ id: 'w-1', name: 'Standup', enabled: true, created_by: 'u-ana' }],
    can_add_webhook: true,
  };
});

describe('ChannelDetailsModal', () => {
  it('gives a moderator About, Settings and Integrations, and everyone else About and Integrations', () => {
    renderModal();
    expect(tabs()).toEqual(['About', 'Settings', 'Integrations']);
    expect(screen.getByTestId('channel-about-name')).toHaveValue('deploys');
  });

  it('shows a member what the channel is about without letting them edit it', () => {
    renderModal({ canModerate: false, initialTab: 'settings' });
    expect(tabs()).toEqual(['About', 'Integrations']);
    expect(screen.getByTestId('channel-about')).toHaveTextContent('Shipping');
    expect(screen.queryByTestId('channel-about-name')).not.toBeInTheDocument();
  });

  it('saves the name, topic and description from About', async () => {
    const onClose = renderModal();
    fireEvent.change(screen.getByTestId('channel-about-topic'), { target: { value: 'Releases' } });
    fireEvent.click(screen.getByTestId('channel-about-save'));
    await waitFor(() =>
      expect(updateChannel).toHaveBeenCalledWith({ name: 'deploys', topic: 'Releases', description: '' }),
    );
    expect(onClose).toHaveBeenCalled();
  });
});

describe('ChannelDetailsModal settings', () => {
  it('shows the workspace policy while channels may not have their own', () => {
    retention = {
      overrides_allowed: false,
      workspace_message_days: 90,
      channel: { message_days: 7, updated_by: null, updated_at: '' },
    };
    renderModal({ initialTab: 'settings' });
    expect(screen.getByTestId('channel-retention-workspace')).toHaveTextContent('kept for 90 days');
    expect(screen.queryByTestId('channel-retention-mode')).not.toBeInTheDocument();
  });

  it('saves who may post and a period of its own', async () => {
    const onClose = renderModal({ initialTab: 'settings' });
    expect(screen.getByRole('option', { name: 'Workspace default (365 days)' })).toBeInTheDocument();

    fireEvent.click(screen.getByTestId('channel-settings-announcement'));
    fireEvent.change(screen.getByTestId('channel-retention-mode'), { target: { value: 'days' } });
    fireEvent.change(screen.getByTestId('channel-retention-days'), { target: { value: '14' } });
    save();

    await waitFor(() => expect(setRetention).toHaveBeenCalledWith(14));
    expect(updateChannel).toHaveBeenCalledWith({ post_policy: 'moderators' });
    expect(onClose).toHaveBeenCalled();
  });

  it('keeps forever, or goes back to the workspace', async () => {
    retention = { ...retention, channel: { message_days: 30, updated_by: null, updated_at: '' } };
    renderModal({ initialTab: 'settings' });
    expect(screen.getByTestId('channel-retention-days')).toHaveValue(30);

    fireEvent.change(screen.getByTestId('channel-retention-mode'), { target: { value: 'forever' } });
    save();
    await waitFor(() => expect(setRetention).toHaveBeenLastCalledWith(null));

    fireEvent.change(screen.getByTestId('channel-retention-mode'), { target: { value: 'workspace' } });
    save();
    await waitFor(() => expect(setRetention).toHaveBeenLastCalledWith('workspace'));
  });

  it('leaves the period alone when it did not change, and refuses less than a day', async () => {
    renderModal({ initialTab: 'settings' });
    save();
    await waitFor(() => expect(updateChannel).toHaveBeenCalled());
    expect(setRetention).not.toHaveBeenCalled();

    fireEvent.change(screen.getByTestId('channel-retention-mode'), { target: { value: 'days' } });
    fireEvent.change(screen.getByTestId('channel-retention-days'), { target: { value: '0' } });
    expect(screen.getByText('At least one day')).toBeInTheDocument();
    expect(screen.getByTestId('channel-settings-save')).toBeDisabled();
  });
});

describe('ChannelDetailsModal integrations', () => {
  it('lists what touches the channel, and only names it to a member', () => {
    integrations = { ...integrations, can_add_webhook: false };
    renderModal({ canModerate: false, initialTab: 'integrations' });

    const incoming = screen.getByTestId('channel-integration-incoming');
    expect(incoming).toHaveTextContent('CI');
    expect(incoming).toHaveTextContent('Added by Ana');
    expect(screen.getByTestId('channel-integration-outgoing')).toHaveTextContent('Mirror');
    expect(screen.getByTestId('channel-integration-command')).toHaveTextContent('/deploy');
    expect(screen.getByTestId('channel-integration-workflow')).toHaveTextContent('On · built by Ana');
    expect(screen.queryByTestId('channel-webhook-add')).not.toBeInTheDocument();
    expect(screen.queryByTestId('channel-webhook-rotate')).not.toBeInTheDocument();
    expect(screen.queryByTestId('channel-integration-workflow-open')).not.toBeInTheDocument();
  });

  it('adds an incoming webhook and shows its URL under it', async () => {
    integrations.incoming_webhooks.push({
      id: 'h-new',
      name: 'Alerts',
      description: null,
      created_by: 'u-ana',
      created_at: '2026-10-26T10:00:00Z',
    });
    renderModal({ initialTab: 'integrations' });
    fireEvent.change(screen.getByTestId('channel-webhook-name'), { target: { value: 'Alerts' } });
    fireEvent.click(screen.getByTestId('channel-webhook-add'));

    await waitFor(() =>
      expect(createHook).toHaveBeenCalledWith({
        hook_type: 'incoming_webhook',
        name: 'Alerts',
        config: { channel_id: 'ch-1' },
      }),
    );
    expect(revealHook).toHaveBeenCalledWith('h-new');
    await waitFor(() => expect(screen.getByTestId('channel-webhook-name')).toHaveValue(''));
    const urls = screen.getAllByTestId('channel-webhook-url-value');
    expect(urls).toHaveLength(1);
    expect(urls[0]).toHaveTextContent('https://chat.test/api/hooks/incoming/first');
  });

  it('makes a new URL for a webhook and shows it under that webhook', async () => {
    renderModal({ initialTab: 'integrations' });
    const incoming = screen.getByTestId('channel-integration-incoming');
    expect(within(incoming).queryByTestId('channel-webhook-url')).not.toBeInTheDocument();

    fireEvent.click(within(incoming).getByTestId('channel-webhook-rotate'));
    await waitFor(() =>
      expect(within(incoming).getByTestId('channel-webhook-url-value')).toHaveTextContent(
        'https://chat.test/api/hooks/incoming/second',
      ),
    );
    expect(rotateHook).toHaveBeenCalledWith('h-ci');
  });

  it('opens a workflow in the editor for someone who may build workflows', () => {
    const onOpenWorkflow = vi.fn();
    const onClose = renderModal({ initialTab: 'integrations', onOpenWorkflow });
    fireEvent.click(screen.getByTestId('channel-integration-workflow-open'));
    expect(onClose).toHaveBeenCalled();
    expect(onOpenWorkflow).toHaveBeenCalledWith('w-1');
  });
});
