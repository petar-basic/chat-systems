import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor, within } from '@testing-library/react';
import IntegrationsPanel from './IntegrationsPanel';
import type { Channel } from '@/stores/workspace';
import type { Hook, HookSecrets } from '@/hooks/queries/useHooks';

const createMutate = vi.fn();
const deleteMutate = vi.fn();
const revealMutate = vi.fn();
const rotateMutate = vi.fn();
let hooks: Hook[] = [];

const hook = (over: Partial<Hook>): Hook => ({
  id: 'hook-1',
  workspace_id: 'ws-1',
  created_by: 'user-1',
  hook_type: 'incoming_webhook',
  name: 'CI alerts',
  description: null,
  config: { channel_id: 'ch-1' },
  is_active: true,
  created_at: '2026-01-01T00:00:00Z',
  ...over,
});

const secrets = (over: Partial<HookSecrets>): HookSecrets => ({
  hook_id: 'hook-1',
  hook_type: 'incoming_webhook',
  config: { token: 'tok-123' },
  incoming_url: 'http://localhost:8080/api/hooks/incoming/tok-123',
  ...over,
});

vi.mock('@/hooks/queries/useHooks', () => ({
  useHooks: () => ({ data: hooks, isLoading: false }),
  useCreateHook: () => ({ mutateAsync: createMutate, isPending: false }),
  useDeleteHook: () => ({ mutateAsync: deleteMutate, isPending: false }),
  useRevealHook: () => ({ mutateAsync: revealMutate, isPending: false }),
  useRotateHook: () => ({ mutateAsync: rotateMutate, isPending: false }),
}));

const channels = [
  { id: 'ch-1', name: 'alerts', channel_type: 'public' },
  { id: 'ch-2', name: 'secret', channel_type: 'private' },
] as Channel[];

function renderPanel() {
  const onClose = vi.fn();
  render(
    <IntegrationsPanel
      workspaceId="ws-1"
      instanceUrl="http://localhost:8080"
      channels={channels}
      onClose={onClose}
    />,
  );
  return { onClose };
}

describe('IntegrationsPanel', () => {
  beforeEach(() => {
    hooks = [];
    createMutate.mockReset().mockImplementation(async () => {
      const created = hook({});
      hooks = [...hooks, created];
      return created;
    });
    deleteMutate.mockReset().mockResolvedValue(undefined);
    revealMutate.mockReset().mockResolvedValue(secrets({}));
    rotateMutate.mockReset().mockResolvedValue(
      secrets({
        config: { token: 'tok-999' },
        incoming_url: 'http://localhost:8080/api/hooks/incoming/tok-999',
      }),
    );
  });

  it('shows the webhook URL right after creating an incoming hook', async () => {
    renderPanel();

    fireEvent.change(screen.getByLabelText('Incoming webhook name'), { target: { value: 'CI alerts' } });
    fireEvent.change(screen.getByLabelText('Incoming webhook channel'), { target: { value: 'ch-1' } });
    fireEvent.click(screen.getByTestId('incoming-hook-create'));

    await waitFor(() =>
      expect(createMutate).toHaveBeenCalledWith({
        hook_type: 'incoming_webhook',
        name: 'CI alerts',
        config: { channel_id: 'ch-1' },
      }),
    );
    expect(await screen.findByText('http://localhost:8080/api/hooks/incoming/tok-123')).toBeInTheDocument();
  });

  it('keeps credentials hidden until the admin asks to reveal them', async () => {
    hooks = [hook({})];
    renderPanel();

    expect(screen.queryByTestId('hook-secrets')).toBeNull();

    fireEvent.click(screen.getByTestId('hook-reveal'));

    await waitFor(() => expect(revealMutate).toHaveBeenCalledWith('hook-1'));
    expect(await screen.findByTestId('hook-secrets')).toBeInTheDocument();
  });

  it('replaces the shown value after rotating', async () => {
    hooks = [hook({})];
    renderPanel();

    fireEvent.click(screen.getByTestId('hook-reveal'));
    expect(await screen.findByText(/tok-123$/)).toBeInTheDocument();

    fireEvent.click(screen.getByTestId('hook-rotate'));

    await waitFor(() => expect(rotateMutate).toHaveBeenCalledWith('hook-1'));
    expect(await screen.findByText(/tok-999$/)).toBeInTheDocument();
    expect(screen.queryByText(/tok-123$/)).toBeNull();
  });

  it('names the channel an incoming hook posts to, and the URL an outgoing one calls', () => {
    hooks = [
      hook({}),
      hook({
        id: 'hook-2',
        hook_type: 'outgoing_webhook',
        name: 'Deploy bot',
        config: { url: 'https://example.com/hooks/chat' },
      }),
    ];
    renderPanel();

    const rows = screen.getAllByTestId('hook-row');
    expect(within(rows[0]).getByTestId('hook-target')).toHaveTextContent('#alerts');
    expect(within(rows[1]).getByTestId('hook-target')).toHaveTextContent('https://example.com/hooks/chat');
  });

  it('asks for confirmation before deleting', async () => {
    hooks = [hook({})];
    renderPanel();

    fireEvent.click(screen.getByTestId('hook-delete'));
    expect(deleteMutate).not.toHaveBeenCalled();

    fireEvent.click(screen.getByTestId('hook-delete-confirm'));
    await waitFor(() => expect(deleteMutate).toHaveBeenCalledWith('hook-1'));
  });

  it('surfaces a failed creation instead of clearing the form', async () => {
    createMutate.mockRejectedValue(new Error('Requires at least Admin role'));
    renderPanel();

    fireEvent.change(screen.getByLabelText('Outgoing webhook name'), { target: { value: 'Deploy bot' } });
    fireEvent.change(screen.getByLabelText('Outgoing webhook URL'), {
      target: { value: 'https://example.com/hooks/chat' },
    });
    fireEvent.click(screen.getByTestId('outgoing-hook-channel-ch-1'));
    fireEvent.click(screen.getByTestId('outgoing-hook-create'));

    expect(await screen.findByText('Requires at least Admin role')).toBeInTheDocument();
    expect(screen.getByLabelText('Outgoing webhook name')).toHaveValue('Deploy bot');
  });

  it('will not create an outgoing webhook until it is scoped to a channel', async () => {
    renderPanel();

    fireEvent.change(screen.getByLabelText('Outgoing webhook name'), { target: { value: 'Deploy bot' } });
    fireEvent.change(screen.getByLabelText('Outgoing webhook URL'), {
      target: { value: 'https://example.com/hooks/chat' },
    });
    expect(screen.getByTestId('outgoing-hook-create')).toBeDisabled();

    fireEvent.click(screen.getByTestId('outgoing-hook-channel-ch-2'));
    fireEvent.click(screen.getByTestId('outgoing-hook-create'));

    await waitFor(() =>
      expect(createMutate).toHaveBeenCalledWith({
        hook_type: 'outgoing_webhook',
        name: 'Deploy bot',
        config: { url: 'https://example.com/hooks/chat', channel_ids: ['ch-2'] },
      }),
    );
  });

  it('says which channels an outgoing webhook forwards', () => {
    hooks = [
      hook({
        id: 'hook-2',
        hook_type: 'outgoing_webhook',
        name: 'Deploy bot',
        config: { url: 'https://example.com/hooks/chat', channel_ids: ['ch-1', 'ch-2'] },
      }),
    ];
    renderPanel();

    expect(screen.getByTestId('hook-scope')).toHaveTextContent('#alerts, #secret');
  });
});
