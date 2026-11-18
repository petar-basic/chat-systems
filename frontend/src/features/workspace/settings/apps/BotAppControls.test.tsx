import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
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
      },
    });
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
    expect(screen.getByText('誰かがこのアプリをメンションした')).toBeInTheDocument();
    expect(screen.getByText('公開チャンネルが作られた')).toBeInTheDocument();
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
