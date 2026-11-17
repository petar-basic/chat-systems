import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor, within } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import i18n from '@/lib/i18n';
import { SlackImportSection } from './SlackImportSection';

const hostile = '<img src=x onerror="alert(1)"> {{name}}';

const get = vi.fn();
const post = vi.fn();
const del = vi.fn();

vi.mock('@/shared/hooks/useCurrentApi', async () => {
  const { typedApiMock } = await import('@/test/typedApiMock');
  return {
    getApiForInstance: () => ({
      ...typedApiMock({
        get: (...args) => get(...args),
        post: (...args) => post(...args),
        delete: (...args) => del(...args),
      }),
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
    archive_deleted_at: null,
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
    del
      .mockReset()
      .mockResolvedValue({ data: run({ status: 'failed', archive_deleted_at: '2026-10-28T10:00:00Z' }) });
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

  it('discards a failed run’s export once the admin confirms', async () => {
    get.mockResolvedValue({ data: [run({ id: 'lost', status: 'failed', retryable: true })] });
    renderSection();

    fireEvent.click(await screen.findByRole('button', { name: 'Discard' }));
    expect(screen.getByText('Discard this export?')).toBeInTheDocument();
    expect(del).not.toHaveBeenCalled();

    fireEvent.click(screen.getByTestId('slack-import-discard-dialog-confirm'));
    await waitFor(() =>
      expect(del).toHaveBeenCalledWith('/workspaces/{ws_id}/slack-imports/{import_id}/archive', undefined),
    );
  });

  it('says why a failed run whose export is gone cannot be started again', async () => {
    get.mockResolvedValue({
      data: [run({ id: 'gone', status: 'failed', archive_deleted_at: '2026-10-28T10:00:00Z' })],
    });
    renderSection();

    expect(await screen.findByTestId('slack-import-archive-gone')).toHaveTextContent(
      "so this run can't be started again",
    );
    expect(screen.queryByRole('button', { name: 'Start again' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Discard' })).not.toBeInTheDocument();
  });
});

describe('SlackImportSection in Japanese', () => {
  beforeEach(async () => {
    get.mockReset();
    del.mockReset().mockResolvedValue({ data: run({ status: 'failed' }) });
    await i18n.changeLanguage('ja');
  });

  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('reports a run, and names its export as text', async () => {
    get.mockResolvedValue({
      data: [
        run({
          source: hostile,
          dry_run: true,
          status: 'failed',
          retryable: true,
          report: {
            messages_imported: 2,
            users_suspended: 1,
            skipped: [
              { what: '#random', why: 'no access' },
              { what: 'a.pdf', why: 'too big' },
            ],
          },
        }),
      ],
    });
    renderSection();

    const row = await screen.findByTestId('slack-import-run');
    expect(within(row).getByTestId('slack-import-source').textContent).toBe(hostile);
    expect(within(row).getByTestId('slack-import-status')).toHaveTextContent('失敗');
    expect(within(row).getByText('ドライラン')).toBeInTheDocument();
    expect(row).toHaveTextContent('メッセージ 2件');
    expect(row).toHaveTextContent('Slack を退出済み 1人');
    expect(within(row).getByText('取り込まなかったもの: 2件')).toBeInTheDocument();
    expect(within(row).getByRole('button', { name: 'やり直す' })).toBeInTheDocument();

    fireEvent.click(within(row).getByRole('button', { name: '破棄' }));
    expect(screen.getByText('このエクスポートを破棄しますか?')).toBeInTheDocument();
    expect(
      screen.getByText(
        `${hostile} を今すぐサーバーから削除します。この取り込みはやり直せなくなります。取り込みを終えるには、エクスポートをもう一度アップロードしてください。`,
      ),
    ).toBeInTheDocument();
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  it('says why a run whose export is gone cannot start again', async () => {
    get.mockResolvedValue({
      data: [run({ status: 'failed', archive_deleted_at: '2026-10-28T10:00:00Z' })],
    });
    renderSection();

    expect(await screen.findByTestId('slack-import-archive-gone')).toHaveTextContent(
      'この取り込みはやり直せません。取り込みを終えるには、もう一度アップロードしてください。',
    );
  });

  it('explains the form, and asks for the export first', async () => {
    get.mockResolvedValue({ data: [] });
    renderSection();

    expect(
      await screen.findByText(
        '取り込みはまだありません。先にドライランをすると、変換できないものを手軽に確かめられます。',
      ),
    ).toBeInTheDocument();
    expect(screen.getByText('ワークスペースのデータのエクスポート').parentElement).toHaveTextContent(
      'Slack のワークスペースのデータのエクスポートで作った .zip ファイルです。',
    );
    expect(screen.getByLabelText('Slack のエクスポート')).toBeInTheDocument();
    expect(screen.getByLabelText('ドライラン')).toBeChecked();

    fireEvent.click(screen.getByRole('button', { name: 'エクスポートを確認' }));
    expect(screen.getByTestId('slack-import-form-error')).toHaveTextContent(
      'Slack から受け取った .zip ファイルを選んでください',
    );
    fireEvent.click(screen.getByLabelText('ドライラン'));
    expect(screen.getByRole('button', { name: '取り込む' })).toBeInTheDocument();
  });
});
