import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor, within } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import LanguagePicker from './LanguagePicker';

const get = vi.fn();
const patch = vi.fn();
const client = {
  typed: async (call: (c: { GET: typeof get; PATCH: typeof patch }) => Promise<{ data: unknown }>) =>
    (await call({ GET: get, PATCH: patch })).data,
};

vi.mock('@/shared/hooks/useCurrentApi', () => ({ getApiForInstance: () => client }));
vi.mock('@/stores/workspace', () => ({
  useWorkspaceStore: (select: (state: unknown) => unknown) =>
    select({ currentWorkspace: { id: 'w-1', instanceUrl: 'http://chat.test' } }),
}));

const saved = (language: unknown) => ({
  data: {
    colour_mode: null,
    density: 'clean',
    previews: 'shown',
    enter_key: 'send',
    open_channel_at: 'where_left',
    language,
  },
});

async function renderPicker() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  render(
    <QueryClientProvider client={queryClient}>
      <LanguagePicker />
    </QueryClientProvider>,
  );
  const picker = screen.getByTestId('language-picker') as HTMLSelectElement;
  await waitFor(() => expect(picker).toBeEnabled());
  return picker;
}

beforeEach(() => {
  get.mockReset();
  patch.mockReset();
});

describe('LanguagePicker', () => {
  it('is its own section of Appearance', async () => {
    get.mockResolvedValue(saved('browser'));
    await renderPicker();
    const section = screen.getByTestId('language');
    expect(within(section).getByRole('heading', { name: 'Language' })).toBeInTheDocument();
    expect(within(section).getByRole('combobox', { name: 'Language' })).toBe(
      screen.getByTestId('language-picker'),
    );
  });

  it('saves the language that is picked', async () => {
    get.mockResolvedValue(saved('browser'));
    patch.mockImplementation(async (_path: string, { body }: { body: { language: string } }) =>
      saved(body.language),
    );
    const picker = await renderPicker();
    expect(picker.value).toBe('browser');

    fireEvent.change(picker, { target: { value: 'ja' } });

    await waitFor(() => expect(picker.value).toBe('ja'));
    expect(patch).toHaveBeenCalledWith('/users/me/preferences', { body: { language: 'ja' } });
  });

  it('goes back to the browser', async () => {
    get.mockResolvedValue(saved('ja'));
    patch.mockResolvedValue(saved('browser'));
    const picker = await renderPicker();
    expect(picker.value).toBe('ja');

    fireEvent.change(picker, { target: { value: 'browser' } });

    await waitFor(() => expect(picker.value).toBe('browser'));
    expect(patch).toHaveBeenCalledWith('/users/me/preferences', { body: { language: 'browser' } });
  });

  it('keeps the saved language and says so when saving fails', async () => {
    get.mockResolvedValue(saved('en'));
    patch.mockRejectedValue(new Error('offline'));
    const picker = await renderPicker();

    fireEvent.change(picker, { target: { value: 'ja' } });

    expect(await screen.findByRole('alert')).toHaveTextContent('Could not save your language.');
    expect(picker.value).toBe('en');
  });

  it('shows a value it does not know as the browser default', async () => {
    get.mockResolvedValue(saved('<b>ja</b>'));
    expect((await renderPicker()).value).toBe('browser');
  });
});
