import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ApiError } from '@/lib/errors';
import { useAppViewsStore, type AppView } from '@/stores/appViews';
import i18n from '@/lib/i18n';
import { AppViewHost } from './AppViewHost';

const calls = { post: vi.fn() };
vi.mock('@/shared/hooks/useCurrentApi', async () => {
  const { typedApiMock } = await import('@/test/typedApiMock');
  const api = () => typedApiMock({ get: async () => [], post: (...args) => calls.post(...args) });
  return { useCurrentApi: api, getApiForInstance: api };
});

const text = (value: string) => ({ type: 'plain_text' as const, text: value });

function view(id: string, title: string, over: Partial<AppView> = {}): AppView {
  return {
    id,
    app_id: 'app-1',
    root_view_id: 'v-request',
    previous_view_id: null,
    hash: `hash-${id}`,
    type: 'modal',
    title: text(title),
    submit: text('Send'),
    close: text('Cancel'),
    blocks: [
      {
        type: 'input',
        block_id: 'what',
        label: text('What'),
        element: { type: 'plain_text_input', action_id: 'text', max_length: 40 },
      },
      {
        type: 'input',
        block_id: 'when',
        optional: true,
        label: text('When'),
        element: { type: 'datepicker', action_id: 'date' },
      },
      {
        type: 'input',
        block_id: 'teams',
        optional: true,
        label: text('Teams'),
        element: {
          type: 'checkboxes',
          action_id: 'picked',
          options: [
            { text: text('Web'), value: 'web' },
            { text: text('Mobile'), value: 'mobile' },
          ],
          initial_options: [{ text: text('Web'), value: 'web' }],
        },
      },
    ],
    ...over,
  };
}

const request = view('v-request', 'Request');
const details = view('v-details', 'Details', {
  previous_view_id: 'v-request',
  submit: null,
  close: null,
  blocks: [
    {
      type: 'input',
      block_id: 'why',
      label: text('Why'),
      element: { type: 'plain_text_input', action_id: 'reason', multiline: true },
    },
  ],
});

const showing = (...views: AppView[]) => act(() => useAppViewsStore.getState().replace('ws-1', views));

function renderHost() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <AppViewHost workspaceId="ws-1" />
    </QueryClientProvider>,
  );
}

const stack = () => useAppViewsStore.getState().stacks['ws-1'] ?? [];

beforeEach(() => {
  calls.post.mockReset();
  calls.post.mockResolvedValue({ status: 'ok' });
  useAppViewsStore.setState({ stacks: {}, values: {} });
});

describe('AppViewHost', () => {
  it('shows nothing until an app opens a view', () => {
    renderHost();
    expect(screen.queryByTestId('app-view')).not.toBeInTheDocument();
  });

  it('shows a view as a modal with its title, inputs and labels', () => {
    showing(request);
    renderHost();
    const modal = screen.getByTestId('app-view');
    expect(within(modal).getByRole('heading', { name: 'Request' })).toBeInTheDocument();
    expect(within(modal).getByLabelText('What')).toHaveAttribute('maxlength', '40');
    expect(within(modal).getByLabelText(/When/)).toHaveAttribute('type', 'date');
    expect(within(modal).getByTestId('view-input-teams')).toHaveTextContent('Teams (optional)');
    expect(within(modal).getByLabelText('Web')).toBeChecked();
    expect(within(modal).getByLabelText('Mobile')).not.toBeChecked();
    expect(within(modal).getByRole('button', { name: 'Send' })).toHaveAttribute('type', 'submit');
    expect(within(modal).getByRole('button', { name: 'Cancel' })).toBeInTheDocument();
    expect(within(modal).queryByRole('button', { name: 'Back' })).not.toBeInTheDocument();
  });

  it('submits every input by block and action, and closes when the app says so', async () => {
    calls.post.mockResolvedValue({ outcome: 'closed', errors: {} });
    showing(request);
    renderHost();
    fireEvent.change(screen.getByLabelText('What'), { target: { value: 'A staging box' } });
    fireEvent.click(screen.getByLabelText('Mobile'));
    fireEvent.click(screen.getByRole('button', { name: 'Send' }));
    await waitFor(() => expect(calls.post).toHaveBeenCalledOnce());
    expect(calls.post).toHaveBeenCalledWith('/views/{view_id}/submit', {
      values: {
        what: { text: ['A staging box'] },
        when: { date: [] },
        teams: { picked: ['web', 'mobile'] },
      },
    });
    await waitFor(() => expect(screen.queryByTestId('app-view')).not.toBeInTheDocument());
    expect(stack()).toEqual([]);
  });

  it('shows the app’s error under its input and drops it once the input changes', async () => {
    calls.post.mockResolvedValue({ outcome: 'errors', errors: { what: 'Say what you need' } });
    showing(request);
    renderHost();
    fireEvent.click(screen.getByRole('button', { name: 'Send' }));
    expect(await screen.findByText('Say what you need')).toHaveAttribute('role', 'alert');
    expect(screen.getByLabelText('What')).toHaveAttribute('aria-invalid', 'true');
    expect(screen.getByTestId('app-view')).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText('What'), { target: { value: 'A box' } });
    expect(screen.queryByText('Say what you need')).not.toBeInTheDocument();
  });

  it('says so when the app does not answer a submission', async () => {
    calls.post.mockRejectedValue(new ApiError(503, 'The app did not respond'));
    showing(request);
    renderHost();
    fireEvent.click(screen.getByRole('button', { name: 'Send' }));
    expect(await screen.findByTestId('app-view-failure')).toHaveTextContent('The app did not respond');
    expect(screen.getByTestId('app-view')).toBeInTheDocument();
  });

  it('keeps what was typed below a pushed view, and goes back to it', async () => {
    showing(request);
    renderHost();
    fireEvent.change(screen.getByLabelText('What'), { target: { value: 'A staging box' } });
    fireEvent.click(screen.getByLabelText('Mobile'));

    showing(request, details);
    expect(screen.getByRole('heading', { name: 'Details' })).toBeInTheDocument();
    expect(screen.queryByLabelText('What')).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Send' })).not.toBeInTheDocument();
    fireEvent.change(screen.getByLabelText('Why'), { target: { value: 'Load tests' } });

    fireEvent.click(screen.getByRole('button', { name: 'Back' }));
    await waitFor(() => expect(calls.post).toHaveBeenCalledWith('/views/{view_id}/close', { all: false }));
    expect(screen.getByRole('heading', { name: 'Request' })).toBeInTheDocument();
    expect(screen.getByLabelText('What')).toHaveValue('A staging box');
    expect(screen.getByLabelText('Mobile')).toBeChecked();
    expect(stack().map((each) => each.id)).toEqual(['v-request']);
  });

  it('keeps values when the app updates the view in place', () => {
    showing(request);
    renderHost();
    fireEvent.change(screen.getByLabelText('What'), { target: { value: 'A staging box' } });
    showing({ ...request, hash: 'hash-2', title: text('Request (step 2)') });
    expect(screen.getByRole('heading', { name: 'Request (step 2)' })).toBeInTheDocument();
    expect(screen.getByLabelText('What')).toHaveValue('A staging box');
  });

  it('closes the whole stack from the corner', async () => {
    showing(request, details);
    renderHost();
    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    await waitFor(() => expect(screen.queryByTestId('app-view')).not.toBeInTheDocument());
    expect(calls.post).toHaveBeenCalledWith('/views/{view_id}/close', { all: true });
    expect(stack()).toEqual([]);
  });

  it('sends a button inside a view with what the inputs hold', async () => {
    showing(
      view('v-request', 'Request', {
        blocks: [
          ...request.blocks,
          {
            type: 'actions',
            block_id: 'more',
            elements: [{ type: 'button', action_id: 'details', text: text('Add details') }],
          },
        ],
      }),
    );
    renderHost();
    fireEvent.change(screen.getByLabelText('What'), { target: { value: 'A box' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add details' }));
    await waitFor(() => expect(calls.post).toHaveBeenCalledOnce());
    expect(calls.post).toHaveBeenCalledWith('/views/{view_id}/actions', {
      block_id: 'more',
      action_id: 'details',
      value: undefined,
      values: { what: { text: ['A box'] }, when: { date: [] }, teams: { picked: ['web'] } },
    });
  });
});

describe('AppViewHost in Japanese', () => {
  beforeEach(async () => {
    await i18n.changeLanguage('ja');
  });

  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('names its back button in Japanese', () => {
    showing(request, details);
    renderHost();

    expect(screen.getByRole('button', { name: '戻る' })).toBeInTheDocument();
  });

  it('says so in Japanese when the app does not answer a submission', async () => {
    calls.post.mockRejectedValue(new ApiError(503, 'The app did not respond'));
    showing(request);
    renderHost();

    fireEvent.click(screen.getByRole('button', { name: 'Send' }));
    expect(await screen.findByTestId('app-view-failure')).toHaveTextContent(
      'アプリから応答がありませんでした',
    );
  });
});
