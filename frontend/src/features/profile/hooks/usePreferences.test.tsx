import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import type { ReactNode } from 'react';
import { renderHook, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import i18n from '@/lib/i18n';
import { usePreferencesSync } from './usePreferences';
import { useThemeStore } from '@/stores/theme';
import { useDisplayStore } from '@/stores/display';

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

const DEFAULTS = {
  colour_mode: null,
  density: 'clean',
  previews: 'shown',
  enter_key: 'send',
  open_channel_at: 'where_left',
  language: 'browser',
};

function run() {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
  renderHook(() => usePreferencesSync(), { wrapper });
}

describe('usePreferencesSync', () => {
  beforeEach(() => {
    get.mockReset();
    patch.mockReset();
    patch.mockImplementation(async (_path: string, { body }: { body: object }) => ({
      data: { ...DEFAULTS, ...body },
    }));
    useThemeStore.getState().setMode('light');
    useDisplayStore.setState({
      density: 'clean',
      previews: 'shown',
      enterKey: 'send',
      openChannelAt: 'where_left',
    });
  });

  it('writes the colour mode this browser had to the server once', async () => {
    get.mockResolvedValue({ data: DEFAULTS });
    run();

    await waitFor(() =>
      expect(patch).toHaveBeenCalledWith('/users/me/preferences', { body: { colour_mode: 'light' } }),
    );
    expect(patch).toHaveBeenCalledTimes(1);
    expect(useThemeStore.getState().mode).toBe('light');
  });

  it('follows the colour mode and display settings kept on the server', async () => {
    get.mockResolvedValue({
      data: {
        colour_mode: 'dark',
        density: 'compact',
        previews: 'collapsed',
        enter_key: 'newline',
        open_channel_at: 'newest',
      },
    });
    run();

    await waitFor(() => expect(useThemeStore.getState().mode).toBe('dark'));
    expect(useDisplayStore.getState()).toMatchObject({
      density: 'compact',
      previews: 'collapsed',
      enterKey: 'newline',
      openChannelAt: 'newest',
    });
    expect(patch).not.toHaveBeenCalled();
  });

  describe('language', () => {
    afterEach(async () => {
      localStorage.clear();
      await i18n.changeLanguage('en');
    });

    it('follows the language kept on the server', async () => {
      get.mockResolvedValue({ data: { ...DEFAULTS, colour_mode: 'light', language: 'ja' } });
      run();

      await waitFor(() => expect(i18n.language).toBe('ja'));
      expect(localStorage.getItem('chat_language')).toBe('ja');
    });

    it('follows the browser for "browser" and for a value it does not know', async () => {
      for (const language of ['browser', '<b>ja</b>', 'xx']) {
        await i18n.changeLanguage('ja');
        get.mockResolvedValue({ data: { ...DEFAULTS, colour_mode: 'light', language } });
        run();

        await waitFor(() => expect(i18n.language, language).toBe('en'));
        expect(localStorage.getItem('chat_language'), language).toBeNull();
      }
    });
  });
});
