import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { ReactNode } from 'react';
import { renderHook } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { typedApiMock } from '@/test/typedApiMock';
import { messagePath, useOpenMessage, type MessageLocation } from './useMessageLocation';

const navigate = vi.fn();
const hereGet = vi.fn();
const elsewhereGet = vi.fn();
const instances = vi.fn();

vi.mock('react-router', () => ({ useNavigate: () => navigate }));
vi.mock('@/shared/hooks/useCurrentApi', () => ({
  useCurrentApi: () => typedApiMock({ get: hereGet }),
  getApiForInstance: (url: string) => {
    instances(url);
    return typedApiMock({ get: elsewhereGet });
  },
}));

const location: MessageLocation = {
  kind: 'channel',
  workspace_id: 'ws-elsewhere',
  channel_id: 'ch-elsewhere',
  joined: true,
};

function openMessage() {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
  return renderHook(() => useOpenMessage(), { wrapper }).result.current;
}

describe('useOpenMessage', () => {
  beforeEach(() => {
    navigate.mockReset();
    hereGet.mockReset();
    elsewhereGet.mockReset();
    instances.mockReset();
  });

  it('locates a message from another server on that server', async () => {
    hereGet.mockRejectedValue(new Error('404'));
    elsewhereGet.mockResolvedValue(location);

    await openMessage()('m-1', false, 'https://other.example');

    expect(instances).toHaveBeenCalledWith('https://other.example');
    expect(hereGet).not.toHaveBeenCalled();
    expect(navigate).toHaveBeenCalledWith(messagePath(location, 'm-1'));
  });

  it('locates a message on the current server when no other is named', async () => {
    hereGet.mockResolvedValue(location);

    await openMessage()('m-1', true);

    expect(elsewhereGet).not.toHaveBeenCalled();
    expect(navigate).toHaveBeenCalledWith(messagePath(location, 'm-1', true));
  });
});
