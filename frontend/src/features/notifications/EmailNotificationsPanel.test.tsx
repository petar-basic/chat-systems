import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import EmailNotificationsPanel from './EmailNotificationsPanel';

const get = vi.fn();
const patch = vi.fn();

vi.mock('@/shared/hooks/useCurrentApi', async () => {
  const { typedApiMock } = await import('@/test/typedApiMock');
  return {
    getApiForInstance: () =>
      typedApiMock({ get: (...args) => get(...args), patch: (...args) => patch(...args) }),
  };
});
vi.mock('@/stores/workspace', () => ({
  useWorkspaceStore: (select: (state: unknown) => unknown) =>
    select({ currentWorkspace: { id: 'ws-1', instanceUrl: 'http://localhost:3000' } }),
}));

describe('EmailNotificationsPanel', () => {
  beforeEach(() => {
    get.mockReset().mockResolvedValue({ mention_emails: true, available: true });
    patch.mockReset().mockResolvedValue({ mention_emails: false, available: true });
  });

  it('turns mention emails off in the current workspace', async () => {
    render(
      <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
        <EmailNotificationsPanel />
      </QueryClientProvider>,
    );

    fireEvent.click(await screen.findByRole('button', { name: 'Turn off' }));

    await waitFor(() => expect(screen.getByRole('button', { name: 'Turn on' })).toBeInTheDocument());
    expect(get).toHaveBeenCalledWith('/workspaces/{ws_id}/notifications/email');
    expect(patch).toHaveBeenCalledWith('/workspaces/{ws_id}/notifications/email', { mention_emails: false });
  });
});
