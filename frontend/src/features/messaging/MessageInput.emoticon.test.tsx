import { describe, it, expect, vi } from 'vitest';
import { fireEvent, render, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { Editor } from '@tiptap/core';
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
vi.mock(import('@/lib/emojiData'), async (original) => ({
  ...(await original()),
  searchEmojis: async () => [{ id: 'smile', native: '😄', name: 'Smile' }],
}));

function composer() {
  const onSend = vi.fn().mockResolvedValue(undefined);
  render(
    <QueryClientProvider client={new QueryClient()}>
      <MessageInput channelName="general" onSend={onSend} />
    </QueryClientProvider>,
  );
  const dom = document.querySelector('.ProseMirror') as HTMLElement & { editor: Editor };
  const type = (text: string) => {
    const { view } = dom.editor;
    for (const char of text) {
      const { from, to } = view.state.selection;
      const handled = view.someProp('handleTextInput', (handle) =>
        handle(view, from, to, char, () => view.state.tr.insertText(char, from, to)),
      );
      if (!handled) view.dispatch(view.state.tr.insertText(char, from, to));
    }
  };
  const pressEnter = () => fireEvent.keyDown(dom, { key: 'Enter' });
  return { dom, onSend, type, pressEnter };
}

describe('an emoticon at the end of a message', () => {
  it('is sent as its emoji when Enter sends', async () => {
    const { onSend, type, pressEnter } = composer();
    type('thanks :)');
    pressEnter();
    await waitFor(() => expect(onSend).toHaveBeenCalledExactlyOnceWith('thanks 🙂'));
  });

  it('leaves Enter to the emoji picker while it offers emoji', async () => {
    const { dom, onSend, type, pressEnter } = composer();
    type('nice :smil');
    await waitFor(() => expect(dom.editor.storage.emojiSuggestion.open).toBe(true));
    pressEnter();
    expect(onSend).not.toHaveBeenCalled();
  });
});
