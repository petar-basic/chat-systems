import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import type { ReactNode } from 'react';
import { render, screen, fireEvent, waitFor, within } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import i18n from '@/lib/i18n';
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

describe('RetentionSection counts', () => {
  beforeEach(() => {
    get.mockReset();
    get.mockImplementation(async (path: string) =>
      path.endsWith('/preview') ? { data: { messages: 1, files: 1 } } : { data: { policy } },
    );
  });

  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('says one message and one file in the singular', async () => {
    render(<RetentionSection workspaceId="ws-1" />, { wrapper });
    await screen.findByTestId('retention-section');

    keepMessagesFor('30');

    await waitFor(() =>
      expect(screen.getByTestId('retention-preview')).toHaveTextContent(
        'About 1 message and 1 file would be removed now.',
      ),
    );
  });

  it('says it all in Japanese', async () => {
    get.mockImplementation(async (path: string) =>
      path.endsWith('/preview') ? { data: { messages: 1234, files: 0 } } : { data: { policy } },
    );
    patch.mockReset().mockImplementation(async (_path: string, { body }: { body: object }) => ({
      data: { policy: { ...policy, ...body } },
    }));
    await i18n.changeLanguage('ja');
    render(<RetentionSection workspaceId="ws-1" />, { wrapper });

    expect(await screen.findByText('どちらも無期限のあいだは、何も削除されません。')).toBeInTheDocument();
    expect(
      screen.getByText(
        '期間を過ぎたメッセージとファイルは、1日1回の処理で完全に削除されます。元に戻すことはできず、変更前に取ったエクスポートだけが唯一の控えになります。',
      ),
    ).toBeInTheDocument();
    expect(screen.getByRole('group', { name: 'ファイルの保存期間' })).toBeInTheDocument();
    expect(
      screen.getByLabelText('チャンネルごとにメッセージの保存期間を決められるようにする'),
    ).toBeInTheDocument();

    const group = screen.getByRole('group', { name: 'メッセージの保存期間' });
    expect(within(group).getByRole('radio', { name: '無期限' })).toBeChecked();
    fireEvent.click(within(group).getByRole('radio', { name: '日数を指定' }));
    fireEvent.change(screen.getByLabelText('メッセージを保存する日数'), { target: { value: '0' } });
    expect(screen.getByText('1 日以上を指定してください')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('メッセージを保存する日数'), { target: { value: '30' } });
    await waitFor(() =>
      expect(screen.getByTestId('retention-preview')).toHaveTextContent(
        'この設定では、現時点でメッセージ約 1,234 件とファイル約 0 件が削除の対象になります。',
      ),
    );

    fireEvent.click(screen.getByRole('button', { name: '保存期間を保存' }));
    expect(await screen.findByText('保存しました')).toBeInTheDocument();
  });
});
