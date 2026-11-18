import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, within, waitFor } from '@testing-library/react';
import i18n from '@/lib/i18n';
import { AppsSection } from './AppsSection';
import type { AppDirectory, InstalledApp } from '@/features/workspace/hooks/useAppDirectory';
import type { BotAppWithToken, NewBotApp } from '@/features/workspace/hooks/useBotApps';

const hostile = '<img src=x onerror="alert(1)"> {{name}}';

const requestMutate = vi.fn();
const createMutate = vi.fn();
const enableMutate = vi.fn();
const rotateMutate = vi.fn();
let directory: AppDirectory;
let appsLoading = false;

vi.mock('@/features/workspace/hooks/useAppDirectory', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/features/workspace/hooks/useAppDirectory')>()),
  useAppDirectory: () => ({ data: directory, isLoading: false, isError: false, refetch: vi.fn() }),
  useRequestApp: () => ({ mutate: requestMutate, isPending: false, error: null }),
}));
vi.mock('@/features/workspace/hooks/useApps', () => ({
  useApps: () => ({
    data: appsLoading ? undefined : { data: [] },
    isLoading: appsLoading,
    isError: false,
    refetch: vi.fn(),
  }),
  useAppActions: () => ({}),
}));
vi.mock('@/features/workspace/hooks/useBotApps', () => ({
  useBotApps: () => ({ data: { data: [] } }),
  useBotAppActions: () => ({ create: { mutateAsync: createMutate, isPending: false, error: null } }),
}));
vi.mock('@/features/workspace/hooks/useHooks', () => ({
  useEnableHook: () => ({ mutate: enableMutate, isPending: false, error: null }),
  useRotateHook: () => ({ mutate: rotateMutate, isPending: false, error: null }),
  useDeleteHook: () => ({ mutateAsync: vi.fn(), isPending: false, error: null }),
}));

const installed = (over: Partial<InstalledApp>): InstalledApp => ({
  kind: 'incoming_webhook',
  id: 'h-ci',
  name: 'CI',
  description: null,
  added_by: 'u-ana',
  added_by_name: 'Ana',
  added_at: '2026-10-29T10:00:00Z',
  permissions: [{ key: 'post', text: 'Anyone with its URL can post messages into its channel' }],
  every_channel: false,
  channels: [{ id: 'ch-1', name: 'deploys' }],
  hidden_channels: 0,
  health: { state: 'ok' },
  app_id: null,
  ...over,
});

const rows = () => screen.getAllByTestId('app-directory-row');

beforeEach(() => {
  appsLoading = false;
  requestMutate.mockReset();
  enableMutate.mockReset();
  createMutate
    .mockReset()
    .mockImplementation(
      async (made: NewBotApp): Promise<BotAppWithToken> =>
        ({ name: made.name, token: 'cbot-once' }) as BotAppWithToken,
    );
  directory = {
    can_manage: true,
    available: [],
    bot_scopes: [
      { key: 'chat:write', text: 'Post, edit and delete its own messages where it is a member' },
      { key: 'users:read', text: 'Look people up in the workspace' },
    ],
    installed: [
      installed({
        hidden_channels: 1,
        health: { state: 'disabled', reason: 'Its URL kept failing', since: '2026-10-29T11:00:00Z' },
      }),
      installed({
        kind: 'workflow',
        id: 'w-1',
        name: 'Standup',
        permissions: [{ key: 'schedule', text: 'Runs on a schedule' }],
      }),
    ],
  };
});

describe('AppsSection', () => {
  it('shows nothing of the directory until the connected apps have loaded too', () => {
    appsLoading = true;
    render(<AppsSection workspaceId="ws-1" />);

    expect(screen.getByTestId('query-loading')).toBeInTheDocument();
    expect(screen.queryByTestId('app-directory-build-open')).not.toBeInTheDocument();
  });

  it('lists everything installed with who added it, where, and whether it works', () => {
    render(<AppsSection workspaceId="ws-1" />);

    expect(rows()).toHaveLength(2);
    const ci = rows()[0];
    expect(ci).toHaveTextContent('CI');
    expect(ci).toHaveTextContent('Incoming webhook · Ana · #deploys, 1 private channel you are not in');
    expect(within(ci).getByTestId('app-health')).toHaveTextContent('Turned off');
    expect(within(rows()[1]).queryByTestId('app-health')).not.toBeInTheDocument();
  });

  it('opens an app page with what it can do, its channels and its health', () => {
    render(<AppsSection workspaceId="ws-1" />);
    fireEvent.click(rows()[0]);

    const page = screen.getByTestId('app-page');
    expect(page).toHaveTextContent('added by Ana');
    expect(screen.getByTestId('app-page-permissions')).toHaveTextContent('Anyone with its URL');
    expect(screen.getByTestId('app-page-channels')).toHaveTextContent(
      '#deploys, 1 private channel you are not in',
    );
    expect(screen.getByTestId('app-page-health')).toHaveTextContent('Its URL kept failing');

    enableMutate.mockImplementation((_id: string, options?: { onSuccess?: (turnedOn: unknown) => void }) =>
      options?.onSuccess?.({
        hook: {},
        secrets: { incoming_url: 'https://chat.example/api/hooks/incoming/tok-new', config: {} },
      }),
    );
    fireEvent.click(screen.getByTestId('app-reinstall'));
    expect(enableMutate.mock.calls[0][0]).toBe('h-ci');
    expect(screen.getByTestId('app-rotated-url')).toHaveTextContent('tok-new');
    expect(screen.getByTestId('app-rotated-url')).toHaveTextContent('The URL it had before no longer works');

    fireEvent.click(screen.getByTestId('app-page-back'));
    expect(rows()).toHaveLength(2);
  });

  it('opens the page a link names', () => {
    render(<AppsSection workspaceId="ws-1" initialApp={{ kind: 'workflow', id: 'w-1' }} />);

    expect(screen.getByTestId('app-page')).toHaveTextContent('Standup');
    expect(screen.getByTestId('app-page-workflow')).toBeInTheDocument();
  });

  it('says so when the linked app is gone', () => {
    render(<AppsSection workspaceId="ws-1" initialApp={{ kind: 'bot_app', id: 'gone' }} />);

    expect(screen.getByTestId('app-directory-missing')).toBeInTheDocument();
    expect(rows()).toHaveLength(2);
  });

  it('shows a member the directory read-only and lets them ask for an app', () => {
    directory = { ...directory, can_manage: false };
    requestMutate.mockImplementation((_req: unknown, options?: { onSuccess?: () => void }) =>
      options?.onSuccess?.(),
    );
    render(<AppsSection workspaceId="ws-1" />);

    expect(screen.queryByTestId('app-directory-build-open')).not.toBeInTheDocument();
    fireEvent.change(screen.getByTestId('app-request-name'), { target: { value: ' PagerDuty ' } });
    fireEvent.change(screen.getByTestId('app-request-reason'), { target: { value: 'On-call alerts' } });
    fireEvent.click(screen.getByTestId('app-request-send'));
    expect(requestMutate.mock.calls[0][0]).toEqual({ name: 'PagerDuty', reason: 'On-call alerts' });
    expect(screen.getByTestId('app-request-sent')).toHaveTextContent('Asked the admins for PagerDuty');

    fireEvent.click(rows()[0]);
    expect(screen.getByTestId('app-page-read-only')).toBeInTheDocument();
    expect(screen.queryByTestId('app-reinstall')).not.toBeInTheDocument();
    expect(screen.queryByTestId('app-remove')).not.toBeInTheDocument();
  });

  it('makes no token for a new app until an admin agrees to its scopes', async () => {
    render(<AppsSection workspaceId="ws-1" />);
    expect(screen.queryByTestId('app-request')).not.toBeInTheDocument();
    fireEvent.click(screen.getByTestId('app-directory-build-open'));
    fireEvent.change(screen.getByLabelText(/App name/), { target: { value: 'Echo' } });
    fireEvent.click(screen.getByTestId('bot-app-scope-users:read'));
    fireEvent.click(screen.getByTestId('bot-app-create'));

    expect(createMutate).not.toHaveBeenCalled();
    const consent = await screen.findByTestId('app-consent-permissions');
    expect(consent).toHaveTextContent('Post, edit and delete its own messages');
    expect(consent).toHaveTextContent('Look people up in the workspace');

    fireEvent.click(screen.getByTestId('app-consent-confirm'));
    await waitFor(() =>
      expect(createMutate).toHaveBeenCalledWith({
        name: 'Echo',
        description: '',
        scopes: ['chat:write', 'users:read'],
      }),
    );
    expect(await screen.findByTestId('bot-app-token-value')).toHaveTextContent('cbot-once');
  });

  it('a new app is not made when the admin does not agree', async () => {
    render(<AppsSection workspaceId="ws-1" />);
    fireEvent.click(screen.getByTestId('app-directory-build-open'));
    fireEvent.change(screen.getByLabelText(/App name/), { target: { value: 'Echo' } });
    fireEvent.click(screen.getByTestId('bot-app-create'));
    await screen.findByTestId('app-consent-permissions');

    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    await waitFor(() => expect(screen.queryByTestId('app-consent-permissions')).not.toBeInTheDocument());
    expect(createMutate).not.toHaveBeenCalled();
  });
});

describe('AppsSection channels in English', () => {
  it('counts more than one hidden channel in the plural', () => {
    directory = { ...directory, installed: [installed({ hidden_channels: 2 })] };
    render(<AppsSection workspaceId="ws-1" />);

    expect(rows()[0]).toHaveTextContent(
      'Incoming webhook · Ana · #deploys, 2 private channels you are not in',
    );
  });
});

describe('AppsSection in Japanese', () => {
  beforeEach(async () => {
    await i18n.changeLanguage('ja');
  });

  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('lists what is installed, and names it all as text', () => {
    directory = {
      ...directory,
      installed: [
        installed({
          name: hostile,
          added_by_name: hostile,
          channels: [{ id: 'ch-1', name: hostile }],
          hidden_channels: 1,
          health: { state: 'disabled', reason: 'Its URL kept failing', since: '2026-10-29T11:00:00Z' },
        }),
        installed({ kind: 'workflow', id: 'w-1', every_channel: true, health: { state: 'failing' } }),
        installed({ kind: 'slash_command', id: 's-1', channels: [], health: { state: 'off' } }),
        installed({ kind: 'outgoing_webhook', id: 'o-1' }),
        installed({ kind: 'bot_app', id: 'b-1' }),
        installed({ kind: 'connected_app', id: 'c-1' }),
      ],
    };
    render(<AppsSection workspaceId="ws-1" />);

    expect(screen.getByText('インストール済み')).toBeInTheDocument();
    expect(
      screen.getByText(
        'このワークスペースにつながっているものすべてです。追加した人、できること、正しく動いているかが分かります。',
      ),
    ).toBeInTheDocument();
    expect(screen.getByText('利用できるアプリ')).toBeInTheDocument();
    expect(screen.getByText('アプリを作る')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '作る' })).toBeInTheDocument();

    const [hook, workflow, command, outgoing, bot, connected] = rows();
    expect(within(hook).getByText(hostile)).toBeInTheDocument();
    expect(hook).toHaveTextContent(
      `Incoming Webhook · ${hostile} · #${hostile}、参加していない非公開チャンネル 1個`,
    );
    expect(within(hook).getByTestId('app-health')).toHaveTextContent('停止中');
    expect(workflow).toHaveTextContent('ワークフロー · Ana · すべてのチャンネル');
    expect(within(workflow).getByTestId('app-health')).toHaveTextContent('エラー発生中');
    expect(command).toHaveTextContent('スラッシュコマンド · Ana · チャンネルなし');
    expect(within(command).getByTestId('app-health')).toHaveTextContent('オフ');
    expect(outgoing).toHaveTextContent('Outgoing Webhook · Ana');
    expect(bot).toHaveTextContent('アプリ · Ana');
    expect(connected).toHaveTextContent('接続アプリ · Ana');
    expect(document.querySelector('[onerror]')).toBeNull();

    fireEvent.click(hook);
    expect(screen.getByTestId('app-page')).toHaveTextContent('Incoming Webhook');
    expect(screen.getByTestId('app-page-channels')).toHaveTextContent(
      `#${hostile}、参加していない非公開チャンネル 1個`,
    );
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  it('says when nothing is installed, or the linked app is gone', () => {
    directory = { ...directory, installed: [] };
    render(<AppsSection workspaceId="ws-1" initialApp={{ kind: 'bot_app', id: 'gone' }} />);

    expect(screen.getByText('まだ何もつながっていません。')).toBeInTheDocument();
    expect(screen.getByTestId('app-directory-missing')).toHaveTextContent(
      'そのアプリは、もうインストールされていません。',
    );
  });

  it('lets a member ask for an app, and names it as text', () => {
    directory = { ...directory, can_manage: false };
    requestMutate.mockImplementation((_req: unknown, options?: { onSuccess?: () => void }) =>
      options?.onSuccess?.(),
    );
    render(<AppsSection workspaceId="ws-1" />);

    expect(screen.getByText('アプリをリクエスト')).toBeInTheDocument();
    expect(
      screen.getByText('アプリを追加できるのは管理者だけです。リクエストは管理者の通知に届きます。'),
    ).toBeInTheDocument();
    expect(screen.getByLabelText(/理由/)).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText('追加してほしいアプリ'), { target: { value: hostile } });
    fireEvent.click(screen.getByRole('button', { name: 'リクエストを送信' }));

    expect(screen.getByTestId('app-request-sent').textContent).toBe(
      `${hostile} を管理者にリクエストしました。`,
    );
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  it('asks before making an app, and hands its token over once', async () => {
    render(<AppsSection workspaceId="ws-1" />);
    fireEvent.click(screen.getByRole('button', { name: '作る' }));

    expect(
      screen.getByText(
        'アプリには、専用のボットユーザーとトークンがあります。読み書きできるのは追加されたチャンネルの中だけで、ここで選んだスコープの範囲に限られます。呼び出す API は、ボット API のガイドで説明しています。',
      ),
    ).toBeInTheDocument();
    expect(screen.getByLabelText(/説明/)).toBeInTheDocument();
    expect(screen.getByText('スコープ')).toBeInTheDocument();
    const name = screen.getByLabelText('アプリの名前');
    expect(name).toHaveAttribute('placeholder', 'デプロイ用ボット');
    fireEvent.change(name, { target: { value: hostile } });
    fireEvent.click(screen.getByRole('button', { name: 'アプリを作成' }));

    expect(await screen.findByText(`${hostile} をインストールしますか?`)).toBeInTheDocument();
    expect(
      screen.getByText(
        '同意すると、トークンが作られます。このトークンで、アプリは次のことができるようになります:',
      ),
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '同意してトークンを作成' }));

    expect(
      await screen.findByText(
        `${hostile} のトークンを今すぐコピーしてください。二度と表示されず、あとから誰も読み出せません。`,
      ),
    ).toBeInTheDocument();
    expect(screen.getByText('ボットのトークン')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '安全な場所に保存しました' })).toBeInTheDocument();
    expect(document.querySelector('[onerror]')).toBeNull();
  });
});

describe('AppsSection app page in Japanese', () => {
  beforeEach(async () => {
    rotateMutate.mockReset();
    await i18n.changeLanguage('ja');
  });

  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('shows an app page, and names it all as text', () => {
    directory = {
      ...directory,
      installed: [
        installed({
          name: hostile,
          added_by_name: hostile,
          app_id: 'b-1',
          health: { state: 'disabled', reason: 'Its URL kept failing', since: '2026-10-29T11:00:00Z' },
        }),
        installed({ kind: 'bot_app', id: 'b-1', name: hostile }),
      ],
    };
    rotateMutate.mockImplementation((_id: string, options?: { onSuccess?: (made: unknown) => void }) =>
      options?.onSuccess?.({ incoming_url: 'https://chat.example/api/hooks/incoming/tok-r', config: {} }),
    );
    render(<AppsSection workspaceId="ws-1" />);
    fireEvent.click(rows()[0]);

    const page = screen.getByTestId('app-page');
    expect(screen.getByRole('button', { name: 'すべてのアプリ' })).toBeInTheDocument();
    expect(page).toHaveTextContent(`Incoming Webhook · ${hostile} さんが追加 · `);
    expect(screen.getByTestId('app-page-owner').textContent).toBe(`${hostile} の一部`);
    expect(screen.getByTestId('app-page-health')).toHaveTextContent(
      /^.+ から停止しています: Its URL kept failing$/,
    );
    expect(screen.getByText('できること')).toBeInTheDocument();
    expect(screen.getByText('チャンネル')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '再インストール' })).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'URL を再発行' }));
    expect(screen.getByText('Webhook の URL')).toBeInTheDocument();
    expect(screen.getByTestId('app-rotated-url')).toHaveTextContent(
      '今すぐコピーしてください。二度と表示されません。古い URL は使えなくなりました。',
    );

    fireEvent.click(screen.getByRole('button', { name: '削除' }));
    expect(screen.getByText(`${hostile} を削除しますか?`)).toBeInTheDocument();
    expect(screen.getByText('すぐに止まります。すでに投稿した内容は残ります。')).toBeInTheDocument();
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  it('hands over a new signing secret when an outgoing webhook is reinstalled', () => {
    directory = {
      ...directory,
      installed: [installed({ kind: 'outgoing_webhook', id: 'o-1', health: { state: 'off' } })],
    };
    enableMutate.mockImplementation((_id: string, options?: { onSuccess?: (turnedOn: unknown) => void }) =>
      options?.onSuccess?.({ hook: {}, secrets: { config: { secret: 's3cret' } } }),
    );
    render(<AppsSection workspaceId="ws-1" />);
    fireEvent.click(rows()[0]);

    expect(screen.getByTestId('app-page-health')).toHaveTextContent('オフになっています。');
    expect(screen.queryByRole('button', { name: 'URL を再発行' })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '再インストール' }));
    expect(screen.getByText('署名シークレット')).toBeInTheDocument();
    expect(screen.getByTestId('app-rotated-url')).toHaveTextContent(
      '今すぐコピーしてください。二度と表示されません。これから送るものは、すべてこのシークレットで署名されます。',
    );
  });

  it('says how it stopped, with and without a time or a reason', () => {
    const cases = [
      [{ state: 'failing' }, 'エラーが発生しています。'],
      [{ state: 'failing', since: '2026-10-29T11:00:00Z' }, / からエラーが発生しています。$/],
      [{ state: 'failing', reason: 'It answered 500' }, 'エラーが発生しています: It answered 500'],
      [{ state: 'disabled' }, '停止しています。'],
      [{ state: 'disabled', since: '2026-10-29T11:00:00Z' }, / から停止しています。$/],
      [{ state: 'disabled', reason: 'It kept failing' }, '停止しています: It kept failing'],
    ] as const;
    for (const [health, text] of cases) {
      directory = { ...directory, installed: [installed({ health })] };
      const { unmount } = render(<AppsSection workspaceId="ws-1" />);
      fireEvent.click(rows()[0]);
      expect(screen.getByTestId('app-page-health')).toHaveTextContent(text);
      unmount();
    }
  });

  it('tells a member who can change it, and where a workflow is changed', () => {
    directory = { ...directory, can_manage: false };
    const { unmount } = render(<AppsSection workspaceId="ws-1" />);
    fireEvent.click(rows()[0]);
    expect(screen.getByTestId('app-page-read-only')).toHaveTextContent(
      '変更や削除ができるのは、ワークスペースの管理者だけです。',
    );
    unmount();

    directory = { ...directory, can_manage: true };
    render(<AppsSection workspaceId="ws-1" initialApp={{ kind: 'workflow', id: 'w-1' }} />);
    expect(screen.getByTestId('app-page-workflow')).toHaveTextContent(
      '変更やオフは「ワークフロー」で行います。',
    );
  });
});
