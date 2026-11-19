import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor, within } from '@testing-library/react';
import i18n from '@/lib/i18n';
import { BotAppControls } from './BotAppControls';
import type { BotApp, BotAppWithToken } from '@/features/workspace/hooks/useBotApps';

type Options = { onSuccess?: (app: BotAppWithToken) => void };

const hostile = '<img src=x onerror="alert(1)"> {{name}}';

const rotateMutate = vi.fn();
const enableMutate = vi.fn();
const removeMutate = vi.fn();
const setScopesMutate = vi.fn();
const configureMutate = vi.fn();
const makeAppTokenMutate = vi.fn();
const saveShortcutsMutate = vi.fn();
const saveDomainsMutate = vi.fn();
const shortcuts = vi.hoisted(() => ({
  listed: [] as Array<Record<string, unknown>>,
  saved: false,
  error: null as unknown,
}));
const domains = vi.hoisted(() => ({ listed: [] as string[], saved: false, error: null as unknown }));

vi.mock('@/features/apps', () => ({
  useBotAppShortcuts: () => ({ data: shortcuts.listed, dataUpdatedAt: 1 }),
  useSaveBotAppShortcuts: () => ({
    mutate: saveShortcutsMutate,
    isPending: false,
    isSuccess: shortcuts.saved,
    error: shortcuts.error,
  }),
  useBotAppUnfurlDomains: () => ({ data: domains.listed, dataUpdatedAt: 1 }),
  useSaveBotAppUnfurlDomains: () => ({
    mutate: saveDomainsMutate,
    isPending: false,
    isSuccess: domains.saved,
    error: domains.error,
  }),
}));

vi.mock('@/features/workspace/hooks/useBotApps', () => ({
  useBotAppActions: () => ({
    rotate: { mutate: rotateMutate, isPending: false, error: null },
    enable: { mutate: enableMutate, isPending: false, error: null },
    remove: { mutateAsync: removeMutate, isPending: false, error: null },
    setScopes: { mutateAsync: setScopesMutate, isPending: false, error: null },
    configure: { mutate: configureMutate, isPending: false, error: null },
    makeAppToken: { mutate: makeAppTokenMutate, isPending: false, error: null },
    revokeAppToken: { mutate: vi.fn(), isPending: false, error: null },
    revealSecret: { mutate: vi.fn(), isPending: false, error: null, data: undefined },
  }),
}));

const SCOPES = [
  { key: 'chat:write', text: 'Post, edit and delete its own messages where it is a member' },
  { key: 'commands', text: 'Own slash commands' },
  { key: 'users:read', text: 'Look people up in the workspace' },
];

const app = (over: Partial<BotApp> = {}): BotApp => ({
  id: 'app-1',
  workspace_id: 'ws-1',
  bot_user_id: 'bot-1',
  name: 'Deployer',
  description: 'Ships things',
  icon_url: null,
  scopes: ['chat:write', 'commands'],
  events: [],
  request_url: null,
  socket_mode: false,
  interactivity: false,
  home_enabled: false,
  has_app_token: false,
  connected_sockets: 0,
  failing_since: null,
  last_failure: null,
  disabled_at: null,
  disabled_reason: null,
  created_by: 'admin-1',
  created_at: '2026-10-29T00:00:00Z',
  updated_at: '2026-10-29T00:00:00Z',
  ...over,
});

const onRemoved = vi.fn();

function renderControls(over: Partial<BotApp> = {}) {
  render(<BotAppControls app={app(over)} scopes={SCOPES} onRemoved={onRemoved} />);
}

describe('BotAppControls', () => {
  beforeEach(() => {
    rotateMutate
      .mockReset()
      .mockImplementation((_id: string, options?: Options) =>
        options?.onSuccess?.({ ...app(), token: 'cbot-rotated' }),
      );
    enableMutate
      .mockReset()
      .mockImplementation((_id: string, options?: Options) =>
        options?.onSuccess?.({ ...app(), token: 'cbot-turned-on' }),
      );
    removeMutate.mockReset().mockResolvedValue({ status: 'deleted' });
    setScopesMutate.mockReset().mockResolvedValue(app());
    configureMutate.mockReset();
    makeAppTokenMutate
      .mockReset()
      .mockImplementation((appId: string, options?: { onSuccess?: (made: unknown) => void }) =>
        options?.onSuccess?.({ app_id: appId, app_token: 'capp-once' }),
      );
    onRemoved.mockReset();
  });

  it('rotates the token and shows the new one once', () => {
    renderControls();
    fireEvent.click(screen.getByTestId('bot-app-rotate'));

    expect(rotateMutate.mock.calls[0][0]).toBe('app-1');
    expect(screen.getByTestId('bot-app-token-value')).toHaveTextContent('cbot-rotated');
    fireEvent.click(screen.getByTestId('bot-app-token-dismiss'));
    expect(screen.queryByText('cbot-rotated')).not.toBeInTheDocument();
  });

  it('turns a stopped app back on with a token shown once', () => {
    renderControls({
      disabled_at: '2026-11-10T09:00:00Z',
      disabled_reason: 'Its creator left the workspace',
    });
    expect(screen.queryByTestId('bot-app-rotate')).not.toBeInTheDocument();
    expect(screen.getByTestId('bot-app-stopped')).toHaveTextContent('new token and signing secret');

    fireEvent.click(screen.getByTestId('bot-app-enable'));

    expect(enableMutate.mock.calls[0][0]).toBe('app-1');
    expect(screen.getByTestId('bot-app-token-value')).toHaveTextContent('cbot-turned-on');
    expect(screen.queryByTestId('bot-app-stopped')).not.toBeInTheDocument();
  });

  it('offers no Turn on to an app that is on', () => {
    renderControls();
    expect(screen.queryByTestId('bot-app-enable')).not.toBeInTheDocument();
  });

  it('asks again before an app gets a scope it did not have', async () => {
    renderControls();
    fireEvent.click(screen.getByTestId('bot-app-scope-edit-users:read'));
    fireEvent.click(screen.getByTestId('bot-app-scopes-save'));

    expect(setScopesMutate).not.toHaveBeenCalled();
    const consent = await screen.findByTestId('app-consent-permissions');
    expect(consent).toHaveTextContent('Look people up in the workspace');
    expect(consent).not.toHaveTextContent('Own slash commands');

    fireEvent.click(screen.getByTestId('app-consent-confirm'));
    await waitFor(() =>
      expect(setScopesMutate).toHaveBeenCalledWith({
        appId: 'app-1',
        scopes: ['chat:write', 'commands', 'users:read'],
      }),
    );
  });

  it('takes a scope away without asking', () => {
    renderControls();
    expect(screen.getByTestId('bot-app-scopes-save')).toBeDisabled();
    fireEvent.click(screen.getByTestId('bot-app-scope-edit-commands'));
    fireEvent.click(screen.getByTestId('bot-app-scopes-save'));

    expect(screen.queryByTestId('app-consent-permissions')).not.toBeInTheDocument();
    expect(setScopesMutate).toHaveBeenCalledWith({ appId: 'app-1', scopes: ['chat:write'] });
  });

  it('removes behind a confirmation', async () => {
    renderControls();
    fireEvent.click(screen.getByTestId('bot-app-remove'));
    expect(removeMutate).not.toHaveBeenCalled();

    fireEvent.click(await screen.findByTestId('bot-app-remove-dialog-confirm'));
    await waitFor(() => expect(onRemoved).toHaveBeenCalled());
    expect(removeMutate).toHaveBeenCalledWith('app-1');
  });

  it('subscribes an app to events delivered to its request URL', () => {
    renderControls();
    fireEvent.click(screen.getByTestId('bot-app-event-app_mention'));
    fireEvent.change(screen.getByLabelText(/Request URL/), {
      target: { value: ' https://bot.example.com/events ' },
    });
    fireEvent.click(screen.getByTestId('bot-app-interactivity'));
    fireEvent.click(screen.getByTestId('bot-app-events-save'));

    expect(configureMutate.mock.calls[0][0]).toEqual({
      appId: 'app-1',
      settings: {
        events: ['app_mention'],
        request_url: 'https://bot.example.com/events',
        socket_mode: false,
        interactivity: true,
        home_enabled: false,
      },
    });
  });

  it('turns the Home tab on and lets the app hear it being opened', () => {
    renderControls();
    fireEvent.click(screen.getByTestId('bot-app-home'));
    fireEvent.click(screen.getByTestId('bot-app-event-app_home_opened'));
    fireEvent.click(screen.getByTestId('bot-app-events-save'));

    expect(configureMutate.mock.calls[0][0].settings).toMatchObject({
      events: ['app_home_opened'],
      home_enabled: true,
    });
  });

  it('saves the shortcuts it adds, in order, and drops the ones removed', () => {
    shortcuts.listed = [
      {
        id: 'sc-1',
        app_id: 'app-1',
        kind: 'global',
        name: 'Old one',
        description: 'Goes away',
        callback_id: 'old',
      },
    ];
    renderControls();
    fireEvent.click(screen.getByTestId('bot-app-shortcut-remove'));
    fireEvent.click(screen.getByTestId('bot-app-shortcut-add'));
    fireEvent.change(screen.getByLabelText('Where'), { target: { value: 'message' } });
    fireEvent.change(screen.getByLabelText('Callback ID'), { target: { value: 'ticket_from_message' } });
    fireEvent.change(screen.getByLabelText('Name'), { target: { value: 'Create ticket from message' } });
    fireEvent.change(screen.getByLabelText('Description'), {
      target: { value: 'Turn this message into a ticket' },
    });
    fireEvent.click(screen.getByTestId('bot-app-shortcuts-save'));

    expect(saveShortcutsMutate).toHaveBeenCalledWith([
      {
        kind: 'message',
        name: 'Create ticket from message',
        description: 'Turn this message into a ticket',
        callback_id: 'ticket_from_message',
      },
    ]);
    shortcuts.listed = [];
  });

  it('claims the domains it previews, trimmed, and drops the ones removed', () => {
    domains.listed = ['old.example.com'];
    saveDomainsMutate.mockReset();
    renderControls({ scopes: ['chat:write', 'links:read', 'links:write'] });
    expect(screen.queryByTestId('bot-app-unfurl-needs-scopes')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Remove old.example.com' }));
    fireEvent.click(screen.getByTestId('bot-app-unfurl-domain-add'));
    fireEvent.click(screen.getByTestId('bot-app-unfurl-domain-add'));
    fireEvent.change(screen.getByLabelText('Domain 1'), { target: { value: ' tracker.example.com ' } });
    fireEvent.click(screen.getByTestId('bot-app-unfurl-domains-save'));

    expect(saveDomainsMutate).toHaveBeenCalledWith(['tracker.example.com']);
    domains.listed = [];
  });

  it('says an app without the links scopes claims no domains', () => {
    renderControls();
    expect(screen.getByTestId('bot-app-unfurl-needs-scopes')).toHaveTextContent(
      'Give the app the links:read and links:write scopes to claim domains.',
    );
    expect(screen.getByTestId('bot-app-unfurl-domain-add')).toBeDisabled();
  });

  it('lets the app hear links to its domains', () => {
    renderControls({ scopes: ['chat:write', 'links:read', 'links:write'] });
    fireEvent.click(screen.getByTestId('bot-app-event-link_shared'));
    fireEvent.click(screen.getByTestId('bot-app-events-save'));

    expect(configureMutate.mock.calls[0][0].settings).toMatchObject({ events: ['link_shared'] });
  });

  it('an app in socket mode gets an app-level token shown once and no request URL', () => {
    renderControls({ socket_mode: true, events: ['message'], connected_sockets: 2 });

    expect(screen.queryByLabelText(/Request URL/)).not.toBeInTheDocument();
    expect(screen.getByTestId('bot-app-socket-count')).toHaveTextContent('2 sockets open');
    fireEvent.click(screen.getByTestId('bot-app-app-token-make'));
    expect(makeAppTokenMutate.mock.calls[0][0]).toBe('app-1');
    expect(screen.getByTestId('bot-app-app-token-value')).toHaveTextContent('capp-once');
  });

  it('an app nothing reaches shows why', () => {
    renderControls({ failing_since: '2026-10-29T10:00:00Z', last_failure: 'The request URL answered 503' });

    expect(screen.getByTestId('bot-app-failing')).toHaveTextContent('answered 503');
  });
});

describe('BotAppControls sockets in English', () => {
  it('counts one open socket in the singular', () => {
    renderControls({ socket_mode: true, connected_sockets: 1 });
    expect(screen.getByTestId('bot-app-socket-count')).toHaveTextContent('1 socket open');
  });
});

describe('BotAppControls in Japanese', () => {
  beforeEach(async () => {
    setScopesMutate.mockReset().mockResolvedValue({});
    await i18n.changeLanguage('ja');
  });

  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('explains a stopped app, its scopes and its events', () => {
    renderControls({
      name: hostile,
      disabled_at: '2026-10-29T00:00:00Z',
      failing_since: '2026-10-29T10:00:00Z',
      last_failure: 'The request URL answered 503',
    });

    expect(screen.getByRole('button', { name: 'オンにする' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '削除' })).toBeInTheDocument();
    expect(screen.getByTestId('bot-app-stopped')).toHaveTextContent(
      'オンにすると、新しいトークンと署名シークレットが発行され、このアプリは、オンにしたあなたの代わりとして動くようになります。前のトークンとシークレットは、使えないままです。',
    );
    expect(screen.getByText('スコープ')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'スコープを保存' })).toBeDisabled();
    expect(screen.getByTestId('bot-app-failing')).toHaveTextContent(
      /^.+ からエラーが発生しています: The request URL answered 503$/,
    );
    expect(screen.getByText('イベント')).toBeInTheDocument();
    expect(screen.getByText('誰かがこのアプリをメンションしたとき')).toBeInTheDocument();
    expect(screen.getByText('公開チャンネルが作られたとき')).toBeInTheDocument();
    expect(screen.getByLabelText('ソケットモード')).not.toBeChecked();
    expect(
      screen.getByText(
        'アプリがこのサーバーにソケットを開き、イベントとスラッシュコマンドをそこで受け取ります。そのため、公開された URL は要りません。',
      ),
    ).toBeInTheDocument();
    expect(screen.getByLabelText(/リクエスト URL/)).toBeInTheDocument();
    expect(screen.getByLabelText('インタラクティブ機能')).not.toBeChecked();
    expect(
      screen.getByText(
        'このアプリのメッセージにあるボタン、メニュー、モーダルの操作が、リクエスト URL に送られます。オフのときは、どれも使えません。',
      ),
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '保存' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '署名シークレットを表示' })).toBeInTheDocument();
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  it('turns it on, and hands the token over once', () => {
    renderControls({ name: hostile, disabled_at: '2026-10-29T00:00:00Z' });
    enableMutate.mockImplementation((_id: string, options?: Options) =>
      options?.onSuccess?.({ ...app({ name: hostile }), token: 'cbot-turned-on' }),
    );
    fireEvent.click(screen.getByRole('button', { name: 'オンにする' }));

    expect(
      screen.getByText(
        `${hostile} のトークンを今すぐコピーしてください。二度と表示されず、あとから誰も読み出せません。`,
      ),
    ).toBeInTheDocument();
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  it('asks before giving more access, and before removing it, naming it as text', async () => {
    renderControls({ name: hostile });
    expect(screen.getByRole('button', { name: 'トークンを再発行' })).toBeInTheDocument();

    fireEvent.click(screen.getByTestId('bot-app-scope-edit-users:read'));
    fireEvent.click(screen.getByRole('button', { name: 'スコープを保存' }));
    expect(await screen.findByText(`${hostile} にできることを増やしますか?`)).toBeInTheDocument();
    expect(
      screen.getByText('トークンはそのまま使えます。これからは、次のこともできるようになります:'),
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '同意して保存' }));
    await waitFor(() => expect(setScopesMutate).toHaveBeenCalled());

    fireEvent.click(screen.getByRole('button', { name: '削除' }));
    expect(await screen.findByText(`${hostile} を削除しますか?`)).toBeInTheDocument();
    expect(
      screen.getByText(
        'トークンはすぐに使えなくなり、このアプリのスラッシュコマンドと Webhook も一緒に削除されます。投稿した内容は残ります。',
      ),
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'アプリを削除' })).toBeInTheDocument();
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  it('counts sockets and hands an app-level token over once', () => {
    renderControls({ socket_mode: true, connected_sockets: 2, has_app_token: true });

    expect(screen.queryByLabelText(/リクエスト URL/)).not.toBeInTheDocument();
    expect(
      screen.getByText(
        'このアプリのメッセージにあるボタン、メニュー、モーダルの操作が、ソケットを通じてアプリに届きます。オフのときは、どれも使えません。',
      ),
    ).toBeInTheDocument();
    expect(screen.getByTestId('bot-app-socket-count')).toHaveTextContent('開いているソケット: 2個');
    expect(screen.getByRole('button', { name: '無効化' })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'アプリレベルトークンを再発行' }));
    expect(
      screen.getByText(
        '今すぐコピーしてください。二度と表示されません。このトークンはソケットを開くためだけのもので、新しく発行すると、古いトークンで開いたソケットはすべて閉じます。',
      ),
    ).toBeInTheDocument();
    expect(screen.getByText('アプリレベルトークン')).toBeInTheDocument();
  });

  it('says when no socket is open, and offers the first app-level token', () => {
    renderControls({ socket_mode: true });

    expect(screen.getByTestId('bot-app-socket-count')).toHaveTextContent('開いているソケットはありません');
    expect(screen.getByRole('button', { name: 'アプリレベルトークンを作成' })).toBeInTheDocument();
  });
});

describe('BotAppControls Home and shortcuts in Japanese', () => {
  beforeEach(async () => {
    saveShortcutsMutate.mockReset();
    shortcuts.listed = [
      {
        id: 'sc-1',
        app_id: 'app-1',
        kind: 'message',
        name: 'Ticket',
        description: 'Make one',
        callback_id: 'ticket',
      },
    ];
    await i18n.changeLanguage('ja');
  });

  afterEach(async () => {
    shortcuts.listed = [];
    shortcuts.saved = false;
    shortcuts.error = null;
    await i18n.changeLanguage('en');
  });

  it('labels the Home tab and its event', () => {
    renderControls();

    expect(screen.getByLabelText('ホームタブ')).not.toBeChecked();
    expect(
      screen.getByText(
        'このアプリとの会話に「ホーム」タブが追加され、アプリはユーザーごとに、その人に合わせたビューを表示できるようになります（アプリが views.publish で表示内容を送ります）。',
      ),
    ).toBeInTheDocument();
    expect(screen.getByText('誰かがこのアプリのホームタブを開いたとき')).toBeInTheDocument();
  });

  it('labels the shortcuts form', () => {
    renderControls();

    const form = screen.getByTestId('bot-app-shortcuts');
    expect(within(form).getByText('ショートカット')).toBeInTheDocument();
    expect(
      within(form).getByText(
        'グローバルショートカットは、メッセージ入力欄のショートカットメニューとクイックスイッチャーに表示されます。メッセージショートカットは、このアプリが参加している会話で、メッセージの「その他の操作」に表示されます。これらを使用すると、アプリに対してコールバック ID が送信されます。それぞれ最大5つまで設定可能です。',
      ),
    ).toBeInTheDocument();
    const where = within(form).getByLabelText('表示する場所');
    expect(where).toHaveValue('message');
    expect(
      within(where)
        .getAllByRole('option')
        .map((option: HTMLElement) => option.textContent),
    ).toEqual(['グローバル', 'メッセージ']);
    expect(within(form).getByLabelText('コールバック ID')).toHaveValue('ticket');
    fireEvent.click(within(form).getByRole('button', { name: 'このショートカットを削除' }));
    fireEvent.click(within(form).getByRole('button', { name: 'ショートカットを追加' }));
    expect(within(form).getByLabelText('名前')).toHaveAttribute('placeholder', '新しい依頼');
    expect(within(form).getByLabelText('説明')).toHaveAttribute('placeholder', 'ヘルプデスクに依頼する');
    expect(within(form).getByRole('button', { name: 'ショートカットを保存' })).toBeInTheDocument();
  });

  it('says it saved', () => {
    shortcuts.saved = true;
    renderControls();
    expect(screen.getByTestId('bot-app-shortcuts-saved')).toHaveTextContent('保存しました');
  });

  it('says what failed when the server gives no message', () => {
    shortcuts.error = {};
    renderControls();
    expect(screen.getByRole('alert')).toHaveTextContent('ショートカットを保存できませんでした');
  });
});

describe('BotAppControls link previews in Japanese', () => {
  beforeEach(async () => {
    saveDomainsMutate.mockReset();
    await i18n.changeLanguage('ja');
  });

  afterEach(async () => {
    domains.listed = [];
    domains.saved = false;
    domains.error = null;
    await i18n.changeLanguage('en');
  });

  it('labels the domains and their event, naming a domain as text', () => {
    domains.listed = [hostile];
    renderControls({ scopes: ['chat:write', 'links:read', 'links:write'] });

    const form = screen.getByTestId('bot-app-unfurl-domains');
    expect(within(form).getByText('リンクのプレビュー')).toBeInTheDocument();
    expect(
      within(form).getByText(
        'これらのドメインおよびそのサブドメインへのリンクは、アプリによってプレビュー表示されます。アプリは link_shared イベントを受け取り、ブロック形式で応答します。最大数は 5 です。',
      ),
    ).toBeInTheDocument();
    expect(within(form).getByLabelText('ドメイン 1')).toHaveValue(hostile);
    expect(document.querySelector('[onerror]')).toBeNull();
    fireEvent.click(within(form).getByRole('button', { name: `${hostile} を削除` }));
    fireEvent.click(within(form).getByRole('button', { name: 'ドメインを追加' }));
    expect(within(form).getByRole('button', { name: '1 を削除' })).toBeInTheDocument();
    expect(within(form).getByRole('button', { name: 'ドメインを保存' })).toBeInTheDocument();
    expect(
      screen.getByText(
        '参加しているチャンネルで、誰かが「リンクのプレビュー」に登録したドメインへのリンクを投稿したとき',
      ),
    ).toBeInTheDocument();
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  it('says an app needs the links scopes', () => {
    renderControls();
    expect(screen.getByTestId('bot-app-unfurl-needs-scopes')).toHaveTextContent(
      'ドメインを登録するには、アプリに links:read と links:write のスコープを付与してください。',
    );
  });

  it('says it saved', () => {
    domains.saved = true;
    renderControls({ scopes: ['chat:write', 'links:read', 'links:write'] });
    expect(screen.getByTestId('bot-app-unfurl-domains-saved')).toHaveTextContent('保存しました');
  });

  it('says what failed when the server gives no message', () => {
    domains.error = {};
    renderControls({ scopes: ['chat:write', 'links:read', 'links:write'] });
    expect(screen.getByRole('alert')).toHaveTextContent('ドメインを保存できませんでした');
  });
});
