import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { ReactNode } from 'react';
import { render, screen, fireEvent, waitFor, within } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { RetentionSection } from './RetentionSection';

const get = vi.fn();
const patch = vi.fn();

vi.mock('@/shared/hooks/useCurrentApi', () => ({
  getApiForInstance: () => ({
    typed: async (call: (c: { GET: typeof get; PATCH: typeof patch }) => Promise<{ data: unknown }>) =>
      (await call({ GET: get, PATCH: patch })).data,
  }),
}));

const policy = {
  workspace_id: 'ws-1',
  message_days: null,
  file_days: null,
  audit_days: 730,
  notification_days: 90,
  channel_overrides: false,
  updated_by: null,
  updated_at: '2026-10-01T00:00:00Z',
};

function wrapper({ children }: { children: ReactNode }) {
  return (
    <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
      {children}
    </QueryClientProvider>
  );
}

function keepMessagesFor(days: string) {
  const group = screen.getByRole('group', { name: 'Keep messages' });
  fireEvent.click(within(group).getByRole('radio', { name: 'For a number of days' }));
  fireEvent.change(screen.getByLabelText('Days to keep messages'), { target: { value: days } });
}

describe('RetentionSection', () => {
  beforeEach(() => {
    get.mockReset();
    patch.mockReset();
    get.mockImplementation(async (path: string) =>
      path.endsWith('/preview') ? { data: { messages: 1234, files: 0 } } : { data: { policy } },
    );
    patch.mockImplementation(async (_path: string, { body }: { body: object }) => ({
      data: { policy: { ...policy, ...body } },
    }));
  });

  it('says what a period would remove now before anything is saved', async () => {
    render(<RetentionSection workspaceId="ws-1" />, { wrapper });
    expect(await screen.findByText('Nothing is removed while both are kept forever.')).toBeInTheDocument();

    keepMessagesFor('30');

    await waitFor(() =>
      expect(screen.getByTestId('retention-preview')).toHaveTextContent(
        'About 1,234 messages and 0 files would be removed now.',
      ),
    );
    const previewCall = get.mock.calls.find(([path]) => path.endsWith('/preview'));
    expect(previewCall?.[1].params.query).toEqual({
      message_days: 30,
      file_days: undefined,
      channel_overrides: undefined,
    });
    expect(patch).not.toHaveBeenCalled();
  });

  it('saves the whole policy, keeping the periods it does not show', async () => {
    render(<RetentionSection workspaceId="ws-1" />, { wrapper });
    await screen.findByTestId('retention-section');

    keepMessagesFor('90');
    fireEvent.click(screen.getByTestId('retention-save'));

    await waitFor(() => expect(patch).toHaveBeenCalledTimes(1));
    expect(patch.mock.calls[0][1].body).toEqual({
      message_days: 90,
      file_days: null,
      audit_days: 730,
      notification_days: 90,
      channel_overrides: false,
    });
    expect(await screen.findByText('Saved')).toBeInTheDocument();
  });

  it('refuses a period shorter than a day', async () => {
    render(<RetentionSection workspaceId="ws-1" />, { wrapper });
    await screen.findByTestId('retention-section');

    keepMessagesFor('0');

    expect(screen.getByText('At least one day')).toBeInTheDocument();
    expect(screen.getByTestId('retention-save')).toBeDisabled();
  });

  it('lets channels have their own period, and counts with them', async () => {
    render(<RetentionSection workspaceId="ws-1" />, { wrapper });
    await screen.findByTestId('retention-section');

    fireEvent.click(screen.getByLabelText('Let channels keep messages for their own period'));
    await waitFor(() =>
      expect(
        get.mock.calls.some(
          ([path, init]) => path.endsWith('/preview') && init.params.query.channel_overrides,
        ),
      ).toBe(true),
    );
    fireEvent.click(screen.getByTestId('retention-save'));

    await waitFor(() => expect(patch).toHaveBeenCalledTimes(1));
    expect(patch.mock.calls[0][1].body).toMatchObject({ message_days: null, channel_overrides: true });
  });
});
