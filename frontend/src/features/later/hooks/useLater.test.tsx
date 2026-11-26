import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import type { ReactNode } from 'react';
import { renderHook, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import i18n from '@/lib/i18n';
import { useLaterActions } from './useLater';

const { typed, toastError } = vi.hoisted(() => ({ typed: vi.fn(), toastError: vi.fn() }));

vi.mock('@/shared/hooks/useCurrentApi', () => ({ getApiForInstance: () => ({ typed }) }));
vi.mock('@/shared/components/Toast', () => ({ toast: { error: toastError, success: vi.fn() } }));
vi.mock('@/lib/logger', () => ({ logger: { error: vi.fn() } }));

const wrapper = ({ children }: { children: ReactNode }) => (
  <QueryClientProvider client={new QueryClient({ defaultOptions: { mutations: { retry: false } } })}>
    {children}
  </QueryClientProvider>
);

describe.each([
  ['en', "Couldn't update Later"],
  ['ja', '「後で」を更新できませんでした'],
])('a Later change that fails (%s)', (language, failed) => {
  beforeEach(async () => {
    typed.mockReset();
    toastError.mockClear();
    await i18n.changeLanguage(language);
  });

  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('says so when the server gives no reason', async () => {
    typed.mockRejectedValue('down');
    const { result } = renderHook(() => useLaterActions('ws'), { wrapper });
    result.current.remove.mutate('item');
    await waitFor(() => expect(toastError).toHaveBeenCalledWith(failed));
  });

  it("shows the server's reason when it gives one", async () => {
    typed.mockRejectedValue(new Error('Reminder is in the past'));
    const { result } = renderHook(() => useLaterActions('ws'), { wrapper });
    result.current.update.mutate({ id: 'item', remindAt: new Date() });
    await waitFor(() => expect(toastError).toHaveBeenCalledWith('Reminder is in the past'));
  });
});
