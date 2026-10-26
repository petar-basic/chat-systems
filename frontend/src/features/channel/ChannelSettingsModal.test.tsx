import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import ChannelSettingsModal from './ChannelSettingsModal';
import type { Channel } from '@/stores/workspace';
import type { ChannelRetention } from '@/features/workspace/hooks/useRetention';

const updateChannel = vi.fn();
const setRetention = vi.fn();
let retention: ChannelRetention;

vi.mock('@/features/channel/hooks/useChannels', () => ({
  useUpdateChannel: () => ({ mutateAsync: updateChannel, isPending: false }),
  useArchiveChannel: () => ({ mutateAsync: vi.fn(), isPending: false }),
}));
vi.mock('@/features/workspace/hooks/useRetention', () => ({
  useChannelRetention: () => ({ data: retention }),
  useSetChannelRetention: () => ({ mutateAsync: setRetention, isPending: false }),
}));

const channel = {
  id: 'ch-1',
  name: 'random',
  topic: '',
  description: '',
  settings: {},
} as unknown as Channel;

function renderModal() {
  const onClose = vi.fn();
  render(
    <ChannelSettingsModal channel={channel} workspaceId="ws-1" onClose={onClose} onArchived={vi.fn()} />,
  );
  return onClose;
}

const save = () => fireEvent.click(screen.getByTestId('channel-settings-save'));

describe('ChannelSettingsModal retention', () => {
  beforeEach(() => {
    updateChannel.mockReset().mockResolvedValue({});
    setRetention.mockReset().mockResolvedValue({});
    retention = { overrides_allowed: true, workspace_message_days: 365, channel: null };
  });

  it('shows the workspace policy while channels may not have their own', () => {
    retention = {
      overrides_allowed: false,
      workspace_message_days: 90,
      channel: { message_days: 7, updated_by: null, updated_at: '' },
    };
    renderModal();
    expect(screen.getByTestId('channel-retention-workspace')).toHaveTextContent('kept for 90 days');
    expect(screen.queryByTestId('channel-retention-mode')).not.toBeInTheDocument();
  });

  it('saves a period of its own with the rest of the settings', async () => {
    const onClose = renderModal();
    expect(screen.getByTestId('channel-retention-mode')).toHaveValue('workspace');
    expect(screen.getByRole('option', { name: 'Workspace default (365 days)' })).toBeInTheDocument();

    fireEvent.change(screen.getByTestId('channel-retention-mode'), { target: { value: 'days' } });
    fireEvent.change(screen.getByTestId('channel-retention-days'), { target: { value: '14' } });
    save();

    await waitFor(() => expect(setRetention).toHaveBeenCalledWith(14));
    expect(onClose).toHaveBeenCalled();
  });

  it('keeps forever, or goes back to the workspace', async () => {
    retention = { ...retention, channel: { message_days: 30, updated_by: null, updated_at: '' } };
    renderModal();
    expect(screen.getByTestId('channel-retention-days')).toHaveValue(30);

    fireEvent.change(screen.getByTestId('channel-retention-mode'), { target: { value: 'forever' } });
    save();
    await waitFor(() => expect(setRetention).toHaveBeenLastCalledWith(null));

    fireEvent.change(screen.getByTestId('channel-retention-mode'), { target: { value: 'workspace' } });
    save();
    await waitFor(() => expect(setRetention).toHaveBeenLastCalledWith('workspace'));
  });

  it('leaves the period alone when it did not change, and refuses less than a day', async () => {
    renderModal();
    save();
    await waitFor(() => expect(updateChannel).toHaveBeenCalled());
    expect(setRetention).not.toHaveBeenCalled();

    fireEvent.change(screen.getByTestId('channel-retention-mode'), { target: { value: 'days' } });
    fireEvent.change(screen.getByTestId('channel-retention-days'), { target: { value: '0' } });
    expect(screen.getByText('At least one day')).toBeInTheDocument();
    expect(screen.getByTestId('channel-settings-save')).toBeDisabled();
  });
});
