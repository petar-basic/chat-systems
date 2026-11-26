import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import type { ReactNode } from 'react';
import { renderHook, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import i18n from '@/lib/i18n';
import { useSidebarActions } from './useSidebarLayout';

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

type Actions = ReturnType<typeof useSidebarActions>;

const changes: [string, (actions: Actions) => void][] = [
  ['creating a section', (a) => a.createSection.mutate('Projects')],
  ['renaming a section', (a) => a.updateSection.mutate({ id: 's-1', name: 'Done' })],
  ['deleting a section', (a) => a.deleteSection.mutate('s-1')],
  ['reordering sections', (a) => a.reorderSections.mutate(['s-2', 's-1'])],
  ['moving a channel', (a) => a.placeChannel.mutate({ channelId: 'ch-1', sectionId: 's-1' })],
  [
    'collapsing a built-in section',
    (a) => a.collapseBuiltin.mutate({ section: 'channels', collapsed: true }),
  ],
];

describe.each([
  ['en', "Couldn't save your sidebar"],
  ['ja', 'サイドバーの変更を保存できませんでした'],
])('changing the sidebar fails (%s)', (language, failed) => {
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
    const { result } = renderHook(() => useSidebarActions('ws-1'), { wrapper });
    change(result.current);
    await waitFor(() => expect(toastError).toHaveBeenCalledWith(failed));
  });

  it("gives the server's reason as it is", async () => {
    typed.mockRejectedValue(new Error(hostile));
    const { result } = renderHook(() => useSidebarActions('ws-1'), { wrapper });
    result.current.createSection.mutate('Projects');
    await waitFor(() => expect(toastError).toHaveBeenCalledWith(hostile));
  });
});
