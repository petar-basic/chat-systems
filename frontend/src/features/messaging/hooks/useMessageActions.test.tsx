import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ReactNode } from 'react';
import { renderHook, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { useWorkspaceStore, type Workspace } from '@/stores/workspace';
import { useMessageActions } from './useMessageActions';

const mutation = vi.hoisted(() => () => ({ mutate: vi.fn(), mutateAsync: vi.fn() }));

vi.mock('@/features/messaging/hooks/useMessages', () => ({
  useEditMessage: mutation,
  useDeleteMessage: mutation,
  useHideLinkPreviews: mutation,
  useRefreshLinkPreviews: mutation,
  useMarkUnread: mutation,
  useReactToMessage: mutation,
  useRemoveReaction: mutation,
  usePinMessage: mutation,
  useSendMessage: mutation,
}));

vi.mock('@/features/instance', () => ({
  useCurrentUser: () => ({ data: { id: 'u-1' } }),
}));

function wrapper({ children }: { children: ReactNode }) {
  return <QueryClientProvider client={new QueryClient()}>{children}</QueryClientProvider>;
}

describe('useMessageActions', () => {
  const writeText = vi.fn<(text: string) => Promise<void>>(async () => {});

  beforeEach(() => {
    vi.stubGlobal('navigator', { ...navigator, clipboard: { writeText } });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    writeText.mockClear();
  });

  it("links a message on its own server, not on the page's origin", async () => {
    useWorkspaceStore.setState({
      currentWorkspace: { id: 'ws-1', instanceUrl: 'https://chat.example' } as Workspace,
    });
    const { result } = renderHook(() => useMessageActions('ch-1', 'draft'), { wrapper });

    result.current.copyLink('m-1');

    await waitFor(() => expect(writeText).toHaveBeenCalledWith('https://chat.example/app/ws-1/ch-1/m-1'));
  });
});
