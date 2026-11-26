import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import type { ReactNode } from 'react';
import { renderHook, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import i18n from '@/lib/i18n';
import { useWorkspaceStore, type Workspace } from '@/stores/workspace';
import { useMarkNotificationsRead, useSetDnd } from './useNotifications';

const { typed, toastError } = vi.hoisted(() => ({ typed: vi.fn(), toastError: vi.fn() }));

vi.mock('@/shared/hooks/useCurrentApi', () => ({
  useCurrentApi: () => ({ typed }),
  getApiForInstance: () => ({ typed }),
}));
vi.mock('@/shared/components/Toast', () => ({ toast: { error: toastError } }));
vi.mock('@/lib/logger', () => ({ logger: { error: vi.fn() } }));

const wrapper = ({ children }: { children: ReactNode }) => (
  <QueryClientProvider client={new QueryClient({ defaultOptions: { mutations: { retry: false } } })}>
    {children}
  </QueryClientProvider>
);

describe.each([
  [
    'en',
    "Couldn't change notifications. Try again.",
    "Couldn't mark the notifications read",
    "We couldn't find that.",
  ],
  [
    'ja',
    '通知設定を変更できませんでした。もう一度お試しください。',
    '通知を既読にできませんでした',
    "We couldn't find that.",
  ],
])('a notification change that fails (%s)', (language, pauseFailed, readFailed, notFound) => {
  beforeEach(async () => {
    typed.mockReset();
    typed.mockRejectedValue(new Error('down'));
    toastError.mockReset();
    useWorkspaceStore.setState({ currentWorkspace: { id: 'ws-1' } as unknown as Workspace });
    await i18n.changeLanguage(language);
  });

  afterEach(async () => {
    useWorkspaceStore.setState({ currentWorkspace: null });
    await i18n.changeLanguage('en');
  });

  it('says notifications could not be paused or resumed', async () => {
    const { result } = renderHook(() => useSetDnd(), { wrapper });
    result.current.mutate('2026-11-26T18:00:00Z');
    await waitFor(() => expect(toastError).toHaveBeenCalledWith(pauseFailed));
    expect(toastError).not.toHaveBeenCalledWith(notFound);
  });

  it('says notifications could not be marked read', async () => {
    const { result } = renderHook(() => useMarkNotificationsRead('ws-1'), { wrapper });
    result.current.mutate(['n-1']);
    await waitFor(() => expect(toastError).toHaveBeenCalledWith(readFailed));
    expect(toastError).not.toHaveBeenCalledWith(notFound);
  });
});
