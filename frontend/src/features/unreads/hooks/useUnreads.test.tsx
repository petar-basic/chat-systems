import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import type { ReactNode } from 'react';
import { renderHook, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import i18n from '@/lib/i18n';
import { useMarkGroupRead, useMarkWorkspaceRead, type UnreadGroup } from './useUnreads';

const { typed, toastError, toastSuccess } = vi.hoisted(() => ({
  typed: vi.fn(),
  toastError: vi.fn(),
  toastSuccess: vi.fn(),
}));

vi.mock('@/shared/hooks/useCurrentApi', () => ({ useCurrentApi: () => ({ typed }) }));
vi.mock('@/shared/components/Toast', () => ({ toast: { error: toastError, success: toastSuccess } }));
vi.mock('@/lib/logger', () => ({ logger: { error: vi.fn() } }));

const group = {
  channel_id: 'ch-1',
  channel_name: 'design',
  channel_type: 'public',
  muted: false,
  unread_count: 1,
  mention_count: 0,
  last_activity_at: '2026-10-21T10:00:00Z',
  messages: [{ id: 'm1' }],
} as unknown as UnreadGroup;

const wrapper = ({ children }: { children: ReactNode }) => (
  <QueryClientProvider client={new QueryClient({ defaultOptions: { mutations: { retry: false } } })}>
    {children}
  </QueryClientProvider>
);

describe.each([
  ['en', "Couldn't mark the channel read", "Couldn't mark everything read", 'Everything is marked read'],
  ['ja', '既読にできませんでした', 'すべてを既読にできませんでした', 'すべて既読にしました'],
])('marking read (%s)', (language, groupFailed, allFailed, allDone) => {
  beforeEach(async () => {
    typed.mockReset();
    toastError.mockClear();
    toastSuccess.mockClear();
    await i18n.changeLanguage(language);
  });

  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('says when a channel could not be marked read', async () => {
    typed.mockRejectedValue(new Error('down'));
    const { result } = renderHook(() => useMarkGroupRead('ws-1'), { wrapper });
    result.current.mutate(group);
    await waitFor(() => expect(toastError).toHaveBeenCalledWith(groupFailed));
  });

  it('says when everything could not be marked read', async () => {
    typed.mockRejectedValue(new Error('down'));
    const { result } = renderHook(() => useMarkWorkspaceRead('ws-1'), { wrapper });
    result.current.mutate();
    await waitFor(() => expect(toastError).toHaveBeenCalledWith(allFailed));
  });

  it('says when everything is marked read', async () => {
    typed.mockResolvedValue({ status: 'read' });
    const { result } = renderHook(() => useMarkWorkspaceRead('ws-1'), { wrapper });
    result.current.mutate();
    await waitFor(() => expect(toastSuccess).toHaveBeenCalledWith(allDone));
    expect(toastError).not.toHaveBeenCalled();
  });
});
