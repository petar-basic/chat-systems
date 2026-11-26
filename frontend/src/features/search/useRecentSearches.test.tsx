import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import type { ReactNode } from 'react';
import { renderHook, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import i18n from '@/lib/i18n';
import { useRecentSearchActions } from './useRecentSearches';

const { typed, toastError } = vi.hoisted(() => ({ typed: vi.fn(), toastError: vi.fn() }));

vi.mock('@/shared/hooks/useCurrentApi', () => ({ getApiForInstance: () => ({ typed }) }));
vi.mock('@/shared/components/Toast', () => ({ toast: { error: toastError } }));
vi.mock('@/lib/logger', () => ({ logger: { error: vi.fn() } }));

const hostile = '<img src=x onerror="alert(1)"> {{name}}';

const wrapper = ({ children }: { children: ReactNode }) => (
  <QueryClientProvider client={new QueryClient({ defaultOptions: { mutations: { retry: false } } })}>
    {children}
  </QueryClientProvider>
);

type Actions = ReturnType<typeof useRecentSearchActions>;

const changes: [string, (actions: Actions) => void][] = [
  ['removing one', (a) => a.forget('r-1')],
  ['clearing them all', (a) => a.clear()],
];

describe.each([
  ['en', "Couldn't change your recent searches"],
  ['ja', '最近の検索を変更できませんでした'],
])('changing recent searches fails (%s)', (language, failed) => {
  beforeEach(async () => {
    typed.mockReset();
    toastError.mockReset();
    await i18n.changeLanguage(language);
  });

  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it.each(changes)('says so after %s', async (_, change) => {
    typed.mockRejectedValue(new Error(''));
    const { result } = renderHook(() => useRecentSearchActions('ws-1'), { wrapper });
    change(result.current);
    await waitFor(() => expect(toastError).toHaveBeenCalledWith(failed));
  });

  it("gives the server's reason as it is", async () => {
    typed.mockRejectedValue(new Error(hostile));
    const { result } = renderHook(() => useRecentSearchActions('ws-1'), { wrapper });
    result.current.clear();
    await waitFor(() => expect(toastError).toHaveBeenCalledWith(hostile));
  });
});
