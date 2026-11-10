import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import i18n from '@/lib/i18n';
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

const hostile = '<img src=x onerror="alert(1)"> {{name}}';

function renderInput(props: { channelName: string; isDm?: boolean }) {
  return render(
    <QueryClientProvider client={new QueryClient()}>
      <MessageInput {...props} onSend={vi.fn()} />
    </QueryClientProvider>,
  );
}

const placeholder = () => document.querySelector('[data-placeholder]')?.getAttribute('data-placeholder');

describe('MessageInput in Japanese', () => {
  beforeEach(async () => {
    await i18n.changeLanguage('ja');
  });

  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('names the channel and the person in its placeholder, as text', async () => {
    const { unmount } = renderInput({ channelName: hostile });
    await waitFor(() => expect(placeholder()).toBe(`#${hostile} へのメッセージ`));
    unmount();
    renderInput({ channelName: hostile, isDm: true });
    await waitFor(() => expect(placeholder()).toBe(`${hostile} へのメッセージ`));
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  it('names its buttons', () => {
    renderInput({ channelName: 'general' });
    for (const name of ['書式を表示', '絵文字を挿入', '送信']) {
      expect(screen.getByRole('button', { name })).toBeInTheDocument();
    }
    expect(screen.getByTestId('composer-send').getAttribute('data-tooltip')).toMatch(/^送信 \(.+\)$/);
    fireEvent.click(screen.getByRole('button', { name: '書式を表示' }));
    expect(screen.getByRole('button', { name: /^太字 \(.+\)$/ })).toBeInTheDocument();
    for (const name of [
      '取り消し線',
      'インラインコード',
      'コードブロック',
      '箇条書き',
      '番号付きリスト',
      '引用',
    ]) {
      expect(screen.getByRole('button', { name })).toBeInTheDocument();
    }
  });
});
