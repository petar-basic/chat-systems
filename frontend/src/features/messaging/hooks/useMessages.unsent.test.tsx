import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { ReactNode } from 'react';
import { renderHook, waitFor, act } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { QUERY_KEYS } from '@/shared/constants';
import { useSendMessage, type MessagesInfiniteData } from './useMessages';
import { useUnsentStore, withUnsent } from '@/stores/unsent';
import { useDraftStore } from '@/stores/drafts';
import { discardUnsent } from '@/lib/unsent';

const post = vi.fn();
const client = {
  typed: async (call: (c: { POST: typeof post }) => Promise<unknown>) => call({ POST: post }),
};

vi.mock('@/shared/hooks/useCurrentApi', () => ({
  getApiForInstance: () => client,
  useCurrentApi: () => client,
}));
vi.mock('@/shared/components/Toast', () => ({ toast: { error: vi.fn(), success: vi.fn() } }));

const file = {
  id: 'file-1',
  filename: 'plan.pdf',
  mime_type: 'application/pdf',
  size_bytes: 10,
  url: 'https://chat.test/plan.pdf',
  width: null,
  height: null,
};

function setup() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  queryClient.setQueryData<MessagesInfiniteData>(QUERY_KEYS.messages('ch-1'), {
    pages: [{ data: [] }],
    pageParams: [undefined],
  });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
  const view = renderHook(() => useSendMessage('ch-1', 'u-1'), { wrapper });
  const shown = () =>
    withUnsent(
      (queryClient.getQueryData<MessagesInfiniteData>(QUERY_KEYS.messages('ch-1'))?.pages ?? []).flatMap(
        (p) => p.data,
      ),
      useUnsentStore.getState().byScope['ch-1'] ?? [],
    );
  return { queryClient, view, shown };
}

describe('a send that fails', () => {
  beforeEach(() => {
    post.mockReset();
    useUnsentStore.setState({ byScope: {} });
    useDraftStore.setState({ drafts: {} });
  });

  it('keeps its row and files through a refetch, and a retry sends the same files', async () => {
    post.mockRejectedValueOnce(new TypeError('Failed to fetch'));
    const { queryClient, view, shown } = setup();

    act(() => view.result.current.mutate({ content: 'the plan', id: 'local-1', attachments: [file] }));
    await waitFor(() => expect(shown()[0]?.failed).toBe(true));

    queryClient.setQueryData<MessagesInfiniteData>(QUERY_KEYS.messages('ch-1'), {
      pages: [{ data: [] }],
      pageParams: [undefined],
    });
    expect(shown().map((m) => [m.id, m.content, m.metadata?.attachments?.[0]?.id])).toEqual([
      ['local-1', 'the plan', 'file-1'],
    ]);

    post.mockResolvedValueOnce({
      id: 'srv-1',
      channel_id: 'ch-1',
      content: 'the plan',
      client_message_id: 'local-1',
    });
    const failed = shown()[0];
    act(() =>
      view.result.current.mutate({
        content: failed.content,
        id: failed.id,
        attachments: failed.metadata?.attachments ?? [],
      }),
    );
    await waitFor(() => expect(shown().map((m) => m.id)).toEqual(['srv-1']));
    const body = (post.mock.calls[1][1] as { body: { attachment_ids: string[]; client_message_id: string } })
      .body;
    expect(body.attachment_ids).toEqual(['file-1']);
    expect(body.client_message_id).toBe('local-1');
  });

  it('puts the text back in the composer when it is discarded', async () => {
    post.mockRejectedValueOnce(new TypeError('Failed to fetch'));
    const { queryClient, view, shown } = setup();
    act(() => view.result.current.mutate({ content: 'keep me', id: 'local-2' }));
    await waitFor(() => expect(shown()[0]?.failed).toBe(true));

    act(() => discardUnsent(queryClient, shown()[0], 'ch-1'));
    expect(shown()).toEqual([]);
    expect(useDraftStore.getState().getDraft('ch-1')).toBe('keep me');
  });
});
