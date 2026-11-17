import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ReactNode } from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import i18n from '@/lib/i18n';
import { toast } from '@/shared/components/Toast';
import type { CustomEmoji } from '@/features/workspace/hooks/useCustomEmoji';
import { CustomEmojiSection } from './CustomEmojiSection';

const hostile = '<img src=x onerror="alert(1)"> {{name}}';

const state = vi.hoisted(() => ({ emojis: [] as CustomEmoji[] }));

vi.mock('@/features/workspace/hooks/useCustomEmoji', async (original) => ({
  ...(await original<typeof import('@/features/workspace/hooks/useCustomEmoji')>()),
  useCustomEmoji: () => ({ data: state.emojis, isLoading: false }),
}));
vi.mock('@/shared/hooks/useCurrentApi', () => ({
  getApiForInstance: () => ({ getValidToken: async () => 'token', typed: vi.fn() }),
}));

const emoji = (over: Partial<CustomEmoji>): CustomEmoji => ({
  id: 'e1',
  name: 'party',
  url: 'http://localhost/party.png',
  created_by: 'u1',
  created_at: '2026-10-01T00:00:00Z',
  ...over,
});

function wrapper({ children }: { children: ReactNode }) {
  return <QueryClientProvider client={new QueryClient()}>{children}</QueryClientProvider>;
}

describe('CustomEmojiSection', () => {
  beforeEach(() => {
    state.emojis = [];
  });

  afterEach(async () => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
    await i18n.changeLanguage('en');
  });

  it('names its form and its emoji in English', () => {
    state.emojis = [emoji({})];
    render(<CustomEmojiSection workspaceId="w1" />, { wrapper });

    expect(screen.getByPlaceholderText('name (used as :name:)')).toBeInTheDocument();
    expect(screen.getByLabelText('Emoji image')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Remove :party:' })).toBeInTheDocument();
  });

  it('says it all in Japanese, and names an emoji as text', () => {
    state.emojis = [emoji({ name: hostile })];
    return i18n.changeLanguage('ja').then(() => {
      render(<CustomEmojiSection workspaceId="w1" />, { wrapper });

      expect(screen.getByLabelText('絵文字の名前')).toHaveAttribute(
        'placeholder',
        '名前(:name: の形で入力します)',
      );
      expect(screen.getByLabelText('絵文字の画像')).toBeInTheDocument();
      expect(screen.getByRole('button', { name: '絵文字を追加' })).toBeDisabled();
      expect(screen.getByText(`:${hostile}:`)).toBeInTheDocument();
      expect(screen.getByRole('button', { name: `:${hostile}: を削除` })).toBeInTheDocument();
      expect(document.querySelector('[onerror]')).toBeNull();
    });
  });

  it('says when there are none in Japanese', async () => {
    await i18n.changeLanguage('ja');
    render(<CustomEmojiSection workspaceId="w1" />, { wrapper });

    const code = screen.getByText(':name:');
    expect(code.tagName).toBe('CODE');
    expect(code.parentElement).toHaveTextContent(
      'カスタム絵文字はまだありません。ここで追加した絵文字は、:name: の形で入力できます。',
    );
  });

  it('says in Japanese that an emoji was added', async () => {
    const success = vi.spyOn(toast, 'success');
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => new Response('{}', { status: 200 })),
    );
    await i18n.changeLanguage('ja');
    render(<CustomEmojiSection workspaceId="w1" />, { wrapper });

    fireEvent.change(screen.getByLabelText('絵文字の名前'), { target: { value: 'party' } });
    fireEvent.change(screen.getByLabelText('絵文字の画像'), {
      target: { files: [new File(['x'], 'party.png', { type: 'image/png' })] },
    });
    fireEvent.click(screen.getByRole('button', { name: '絵文字を追加' }));

    await waitFor(() => expect(success).toHaveBeenCalledWith('絵文字を追加しました。'));
  });
});
