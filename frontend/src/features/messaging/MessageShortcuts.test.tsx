import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import type { Message } from '@/stores/workspace';
import { MessageShortcutItems } from './MessageShortcuts';

const apps = vi.hoisted(() => ({
  shortcuts: [] as Array<Record<string, unknown>>,
  run: vi.fn(),
  listedFor: [] as Array<string | undefined>,
}));

vi.mock('@/features/apps', () => ({
  useMessageShortcuts: (channelId: string | undefined) => {
    apps.listedFor.push(channelId);
    return { data: channelId ? apps.shortcuts : undefined };
  },
  useRunAppShortcut: () => ({ mutateAsync: apps.run }),
}));

const toastError = vi.hoisted(() => vi.fn());
vi.mock('@/shared/components/Toast', () => ({ toast: { error: toastError, success: vi.fn() } }));

const ticket = {
  id: 'sc-1',
  app_id: 'app-1',
  kind: 'message',
  name: 'Create ticket from message',
  description: 'Turn this message into a ticket',
  callback_id: 'ticket_from_message',
};

const message = { id: 'm-1', channel_id: 'ch-1', pending: false } as unknown as Message;

describe('MessageShortcutItems', () => {
  beforeEach(() => {
    apps.shortcuts = [ticket];
    apps.listedFor = [];
    apps.run.mockReset();
    apps.run.mockResolvedValue({ status: 'ok' });
    toastError.mockReset();
  });

  it('lists the shortcuts of the apps in the conversation and sends the message to the one used', async () => {
    const onDone = vi.fn();
    render(<MessageShortcutItems message={message} onDone={onDone} />);

    expect(apps.listedFor).toContain('ch-1');
    fireEvent.click(screen.getByRole('button', { name: 'Create ticket from message' }));

    expect(onDone).toHaveBeenCalled();
    await waitFor(() => expect(apps.run).toHaveBeenCalledWith({ shortcutId: 'sc-1', messageId: 'm-1' }));
    expect(toastError).not.toHaveBeenCalled();
  });

  it('says so when the app does not answer', async () => {
    apps.run.mockRejectedValue(new Error('The app did not respond'));
    render(<MessageShortcutItems message={message} onDone={vi.fn()} />);

    fireEvent.click(screen.getByRole('button', { name: 'Create ticket from message' }));

    await waitFor(() => expect(toastError).toHaveBeenCalled());
  });

  it('shows nothing where no app has a message shortcut, or on a message still sending', () => {
    apps.shortcuts = [];
    const { unmount } = render(<MessageShortcutItems message={message} onDone={vi.fn()} />);
    expect(screen.queryByTestId('message-shortcuts')).not.toBeInTheDocument();
    unmount();

    apps.shortcuts = [ticket];
    render(<MessageShortcutItems message={{ ...message, pending: true }} onDone={vi.fn()} />);
    expect(screen.queryByTestId('message-shortcuts')).not.toBeInTheDocument();
  });
});
