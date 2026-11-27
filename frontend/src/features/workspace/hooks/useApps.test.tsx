import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import type { ReactNode } from 'react';
import { renderHook, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import i18n from '@/lib/i18n';
import { useAppActions } from './useApps';

const { typed, toastError } = vi.hoisted(() => ({ typed: vi.fn(), toastError: vi.fn() }));

vi.mock('@/shared/hooks/useCurrentApi', () => ({ useCurrentApi: () => ({ typed }) }));
vi.mock('@/shared/components/Toast', () => ({ toast: { error: toastError } }));
vi.mock('@/lib/logger', () => ({ logger: { error: vi.fn() } }));

const hostile = '<img src=x onerror="alert(1)"> {{name}}';

const wrapper = ({ children }: { children: ReactNode }) => (
  <QueryClientProvider client={new QueryClient({ defaultOptions: { mutations: { retry: false } } })}>
    {children}
  </QueryClientProvider>
);

type Actions = ReturnType<typeof useAppActions>;

const changes: [string, (actions: Actions) => void][] = [
  ['removing the app', (a) => a.remove.mutate('github')],
  ['disconnecting an account', (a) => a.disconnect.mutate('github')],
  ['turning prompts off', (a) => a.prompts.mutate({ provider: 'github', off: true })],
];

describe.each([
  ['en', "Couldn't save the app", "Couldn't start connecting the app"],
  ['ja', 'アプリを変更できませんでした', 'アカウントの接続を開始できませんでした'],
])('an app change that fails (%s)', (language, saveFailed, connectFailed) => {
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
    const { result } = renderHook(() => useAppActions('ws-1'), { wrapper });
    change(result.current);
    await waitFor(() => expect(toastError).toHaveBeenCalledWith(saveFailed));
  });

  it('says so when connecting an account cannot start', async () => {
    typed.mockRejectedValue(new Error(''));
    const { result } = renderHook(() => useAppActions('ws-1'), { wrapper });
    result.current.connect.mutate({ provider: 'github', returnTo: '/app' });
    await waitFor(() => expect(toastError).toHaveBeenCalledWith(connectFailed));
  });

  it("gives the server's reason as it is", async () => {
    typed.mockRejectedValue(new Error(hostile));
    const { result } = renderHook(() => useAppActions('ws-1'), { wrapper });
    result.current.connect.mutate({ provider: 'github', returnTo: '/app' });
    await waitFor(() => expect(toastError).toHaveBeenCalledWith(hostile));
  });
});
