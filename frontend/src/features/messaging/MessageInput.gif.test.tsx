import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { Editor } from '@tiptap/core';
import { SETTINGS, answer, giphyPage } from '@/test/giphy';
import i18n from '@/lib/i18n';
import MessageInput from './MessageInput';

let gifSettings: typeof SETTINGS | null = SETTINGS;

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
vi.mock(import('@/features/gifs'), async (original) => ({
  ...(await original()),
  useGifSettings: () => ({ data: gifSettings }) as never,
}));

function composer() {
  const onSend = vi.fn().mockResolvedValue(undefined);
  render(
    <QueryClientProvider client={new QueryClient()}>
      <MessageInput channelName="general" workspaceId="ws-1" gifs onSend={onSend} />
    </QueryClientProvider>,
  );
  const dom = document.querySelector('.ProseMirror') as HTMLElement & { editor: Editor };
  const type = (text: string) => {
    const { view } = dom.editor;
    view.dispatch(view.state.tr.insertText(text));
  };
  const pressEnter = () => fireEvent.keyDown(dom, { key: 'Enter' });
  const text = () => dom.editor.storage.markdown.getMarkdown().trim();
  return { onSend, type, pressEnter, text };
}

beforeEach(() => {
  gifSettings = SETTINGS;
  vi.stubGlobal(
    'fetch',
    vi.fn(() => answer(giphyPage(['cat1', 'cat2']))),
  );
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('GIFs in the message box', () => {
  it('opens the picker on /giphy and sends the GIF picked without the command', async () => {
    const { onSend, type, pressEnter, text } = composer();
    type('/giphy happy cat');
    pressEnter();

    const picker = await screen.findByTestId('gif-picker');
    expect(onSend).not.toHaveBeenCalled();
    expect(screen.getByTestId('gif-search')).toHaveValue('happy cat');

    const [first] = await within(picker).findAllByTestId('gif-choice');
    fireEvent.click(first!);

    await waitFor(() =>
      expect(onSend).toHaveBeenCalledExactlyOnceWith('', expect.objectContaining({ id: 'cat1' })),
    );
    expect(text()).toBe('');
    expect(screen.queryByTestId('gif-picker')).not.toBeInTheDocument();
  });

  it('keeps the /giphy words when the picker is closed without a pick', async () => {
    const { onSend, type, pressEnter, text } = composer();
    type('/giphy happy cat');
    pressEnter();
    await screen.findByTestId('gif-picker');

    fireEvent.keyDown(document, { key: 'Escape' });

    await waitFor(() => expect(screen.queryByTestId('gif-picker')).not.toBeInTheDocument());
    expect(text()).toBe('/giphy happy cat');
    expect(onSend).not.toHaveBeenCalled();
  });

  it('sends what was typed with a GIF picked from the button', async () => {
    const { onSend, type } = composer();
    type('look at this');
    fireEvent.click(screen.getByTestId('composer-gif'));

    const [, second] = await within(await screen.findByTestId('gif-picker')).findAllByTestId('gif-choice');
    fireEvent.click(second!);

    await waitFor(() =>
      expect(onSend).toHaveBeenCalledExactlyOnceWith('look at this', expect.objectContaining({ id: 'cat2' })),
    );
  });

  it('sends /giphy as text when GIFs are off', async () => {
    gifSettings = null;
    const { onSend, type, pressEnter } = composer();
    expect(screen.queryByTestId('composer-gif')).not.toBeInTheDocument();
    type('/giphy cat');
    pressEnter();
    await waitFor(() => expect(onSend).toHaveBeenCalledExactlyOnceWith('/giphy cat'));
  });
});

describe('GIFs in the message box in Japanese', () => {
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('names the GIF button', async () => {
    await i18n.changeLanguage('ja');
    composer();
    expect(screen.getByTestId('composer-gif')).toHaveAttribute('aria-label', 'GIF を送信');
    expect(screen.getByTestId('composer-gif')).toHaveAttribute('data-tooltip', 'GIF を送信');
  });
});
