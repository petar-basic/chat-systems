import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { SlackImportSection } from './SlackImportSection';

const get = vi.fn();
const post = vi.fn();

vi.mock('@/shared/hooks/useCurrentApi', async () => {
  const { typedApiMock } = await import('@/test/typedApiMock');
  return {
    getApiForInstance: () => ({
      ...typedApiMock({ get: (...args) => get(...args), post: (...args) => post(...args) }),
      upload: vi.fn(),
    }),
  };
});

function run(overrides: Record<string, unknown>) {
  return {
    id: 'run-1',
    workspace_id: 'ws-1',
    source: 'acme.zip',
    dry_run: false,
    report: {},
    status: 'complete',
    requested_by: null,
    error: null,
    retryable: false,
    started_at: '2026-10-27T10:00:00Z',
    finished_at: null,
    ...overrides,
  };
}

function renderSection() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={client}>
      <SlackImportSection workspaceId="ws-1" instanceUrl="http://localhost:3000" />
    </QueryClientProvider>,
  );
}

describe('SlackImportSection', () => {
  beforeEach(() => {
    get.mockReset();
    post.mockReset().mockResolvedValue({ data: run({ status: 'pending' }) });
  });

  it('offers to start a run the worker lost again', async () => {
    get.mockResolvedValue({
      data: [
        run({
          id: 'lost',
          status: 'failed',
          error: 'The import stopped when the worker did. Start it again to finish it.',
          retryable: true,
          report: { guests: 2, users_suspended: 1, channels_archived: 3 },
        }),
        run({ id: 'done', source: 'older.zip' }),
      ],
    });
    renderSection();

    const retry = await screen.findByRole('button', { name: 'Start again' });
    expect(screen.getAllByRole('button', { name: 'Start again' })).toHaveLength(1);
    expect(screen.getByText('who left Slack')).toBeInTheDocument();
    expect(screen.getByText('archived')).toBeInTheDocument();

    fireEvent.click(retry);
    await waitFor(() =>
      expect(post).toHaveBeenCalledWith('/workspaces/{ws_id}/slack-imports/{import_id}/retry', undefined),
    );
  });
});
