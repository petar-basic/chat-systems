import { afterEach, describe, expect, it, vi } from 'vitest';
import { render, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { Editor } from '@tiptap/core';
import { useDraftStore } from '@/stores/drafts';
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

const SAFARI =
  'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15';

afterEach(() => {
  vi.restoreAllMocks();
  useDraftStore.setState({ drafts: {} });
});

function mount(props: Partial<Parameters<typeof MessageInput>[0]>) {
  render(
    <QueryClientProvider client={new QueryClient()}>
      <MessageInput onSend={vi.fn().mockResolvedValue(undefined)} {...props} />
    </QueryClientProvider>,
  );
  return document.querySelector('.ProseMirror') as HTMLElement & { editor: Editor };
}

async function expectCaretAfter(dom: HTMLElement & { editor: Editor }, lastLine: string) {
  await waitFor(() => expect(document.activeElement).toBe(dom));
  const { $head } = dom.editor.state.selection;
  expect($head.parent.textContent).toBe(lastLine);
  expect($head.parentOffset).toBe($head.parent.content.size);
}

describe.each([
  ['an iPhone', 'iPhone'],
  ['a desktop', 'MacIntel'],
])('editing a message that ends in a list on %s', (_, platform) => {
  it('opens with the caret after its last word', async () => {
    vi.spyOn(navigator, 'platform', 'get').mockReturnValue(platform);
    const dom = mount({
      editing: true,
      initialContent: 'Hold **bold** text\n\n- copy me\n- with mention',
      onCancel: vi.fn(),
    });
    await expectCaretAfter(dom, 'with mention');
  });
});

describe('a thread reply box opening on desktop Safari', () => {
  it('puts the caret after a draft that ends in a list', async () => {
    vi.spyOn(navigator, 'platform', 'get').mockReturnValue('MacIntel');
    vi.spyOn(navigator, 'userAgent', 'get').mockReturnValue(SAFARI);
    useDraftStore.getState().setDraft('thread:focus', '- first\n- second');
    const dom = mount({ autoFocus: true, draftKey: 'thread:focus' });
    await expectCaretAfter(dom, 'second');
  });
});
