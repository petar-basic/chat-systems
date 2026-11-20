import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import i18n from '@/lib/i18n';
import { GiphyKeySetting } from './GiphyKeySetting';

const hostile = '<img src=x onerror="alert(1)"> {{name}}';

const settings = vi.hoisted(() => ({
  data: { giphy_api_key: 'abc123' } as Record<string, unknown> | undefined,
  mutate: vi.fn(),
  isError: false,
  error: null as unknown,
}));

vi.mock('@/features/instance', () => ({
  useInstanceSettings: () => ({ data: settings.data }),
  useUpdateInstanceSettings: () => ({
    mutate: settings.mutate,
    isPending: false,
    isError: settings.isError,
    error: settings.error,
  }),
}));

function renderSetting() {
  render(
    <QueryClientProvider client={new QueryClient()}>
      <GiphyKeySetting instanceUrl="https://chat.example.com" />
    </QueryClientProvider>,
  );
}

describe('GiphyKeySetting', () => {
  beforeEach(() => {
    settings.data = { giphy_api_key: 'abc123' };
    settings.mutate.mockReset();
    settings.isError = false;
    settings.error = null;
  });

  it('saves a new key and removes the saved one', () => {
    renderSetting();
    fireEvent.change(screen.getByLabelText('GIPHY API key'), { target: { value: 'def456' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save key' }));
    expect(settings.mutate.mock.calls[0][0]).toEqual({ giphy_api_key: 'def456' });

    fireEvent.click(screen.getByRole('button', { name: 'Remove key' }));
    expect(settings.mutate.mock.calls[1][0]).toEqual({ giphy_api_key: '' });
  });

  describe('in Japanese', () => {
    beforeEach(async () => {
      await i18n.changeLanguage('ja');
    });

    afterEach(async () => {
      await i18n.changeLanguage('en');
    });

    it('labels the key and says who gets it, showing a saved key as text', () => {
      settings.data = { giphy_api_key: hostile };
      renderSetting();

      expect(screen.getByLabelText('GIPHY の API キー')).toHaveValue(hostile);
      expect(
        screen.getByText(
          'GIF をオフにしていないすべてのワークスペースで、GIF が使えるようになります。GIPHY の規約により、GIF の検索と読み込みは各ブラウザが GIPHY に直接行います。そのため、このキーはそれらのワークスペースのメンバーに渡されます。キーは developers.giphy.com で取得できます。',
        ),
      ).toBeInTheDocument();
      expect(screen.getByRole('button', { name: 'キーを保存' })).toBeInTheDocument();
      expect(screen.getByRole('button', { name: 'キーを削除' })).toBeInTheDocument();
      expect(document.querySelector('[onerror]')).toBeNull();
    });

    it('says the key could not be saved when the server gives no reason', () => {
      settings.isError = true;
      settings.error = {};
      renderSetting();
      expect(screen.getByRole('alert')).toHaveTextContent('設定を保存できませんでした');
    });
  });
});

describe('the GIF strings', () => {
  function leaves(value: unknown, prefix = ''): string[] {
    if (typeof value !== 'object' || value === null) return [prefix];
    return Object.entries(value).flatMap(([key, child]) => leaves(child, prefix ? `${prefix}.${key}` : key));
  }

  it.each([
    ['admin', 'giphy'],
    ['messages', 'gifs'],
  ])('has Japanese for every one in %s:%s', (ns, group) => {
    const english = leaves(i18n.getResource('en', ns, group));
    const japanese = leaves(i18n.getResource('ja', ns, group));
    expect(english.length).toBeGreaterThan(0);
    expect(japanese).toEqual(english);
  });
});
