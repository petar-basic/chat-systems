import { describe, it, expect, vi } from 'vitest';
import { render, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import MessageInput from './MessageInput';

vi.mock('./hooks/useSlashCommands', () => ({ useSlashCommands: () => ({ data: [] }) }));
vi.mock('@/features/messaging/hooks/useScheduledMessages', () => ({
  useScheduleMessage: () => ({ mutateAsync: vi.fn() }),
}));
vi.mock('@/features/workspace', () => ({ useUserGroups: () => ({ data: [] }) }));
vi.mock('@/features/polls', () => ({
  PollDialog: () => null,
  parsePollCommand: () => null,
  useCreatePoll: () => ({ mutateAsync: vi.fn() }),
}));
vi.mock('@/features/workflows', () => ({ ShortcutMenu: () => null }));

const queryClient = new QueryClient();
const onSend = vi.fn();

function input(props: { channelName: string; isDm?: boolean }) {
  return (
    <QueryClientProvider client={queryClient}>
      <MessageInput {...props} onSend={onSend} />
    </QueryClientProvider>
  );
}

const placeholder = () => document.querySelector('[data-placeholder]')?.getAttribute('data-placeholder');
const editorElement = () => document.querySelector('.ProseMirror');

describe('MessageInput placeholder', () => {
  it('names the person once their name arrives after the composer opened', async () => {
    const { rerender } = render(input({ channelName: 'Unknown user', isDm: true }));
    await waitFor(() => expect(placeholder()).toBe('Message Unknown user'));
    const before = editorElement();

    rerender(input({ channelName: 'Ana', isDm: true }));

    await waitFor(() => expect(placeholder()).toBe('Message Ana'));
    expect(editorElement()).toBe(before);
  });

  it('follows a channel that is renamed', async () => {
    const { rerender } = render(input({ channelName: 'general' }));
    await waitFor(() => expect(placeholder()).toBe('Message #general'));

    rerender(input({ channelName: 'announcements' }));

    await waitFor(() => expect(placeholder()).toBe('Message #announcements'));
  });
});
