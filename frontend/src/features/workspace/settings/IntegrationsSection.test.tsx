import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor, within } from '@testing-library/react';
import { MemoryRouter } from 'react-router';
import i18n from '@/lib/i18n';
import { IntegrationsSection } from './IntegrationsSection';
import type { Channel } from '@/stores/workspace';
import type { Hook, HookSecrets } from '@/features/workspace/hooks/useHooks';

const createMutate = vi.fn();
const deleteMutate = vi.fn();
const revealMutate = vi.fn();
const rotateMutate = vi.fn();
const enableMutate = vi.fn();
let hooks: Hook[] = [];

const hook = (over: Partial<Hook>): Hook => ({
  id: 'hook-1',
  workspace_id: 'ws-1',
  created_by: 'user-1',
  hook_type: 'incoming_webhook',
  name: 'CI alerts',
  description: null,
  config: { channel_id: 'ch-1' },
  is_active: true,
  created_at: '2026-01-01T00:00:00Z',
  updated_at: '2026-01-01T00:00:00Z',
  ...over,
});

const secrets = (over: Partial<HookSecrets>): HookSecrets => ({
  hook_id: 'hook-1',
  hook_type: 'incoming_webhook',
  config: { token: 'tok-123' },
  incoming_url: 'http://localhost:8080/api/hooks/incoming/tok-123',
  ...over,
});

vi.mock('@/features/workspace/hooks/useHooks', () => ({
  useHooks: () => ({ data: hooks, isLoading: false }),
  useCreateHook: () => ({ mutateAsync: createMutate, isPending: false }),
  useDeleteHook: () => ({ mutateAsync: deleteMutate, isPending: false }),
  useRevealHook: () => ({ mutateAsync: revealMutate, isPending: false }),
  useRotateHook: () => ({ mutateAsync: rotateMutate, isPending: false }),
  useEnableHook: () => ({ mutateAsync: enableMutate, isPending: false }),
}));

const channels = [
  { id: 'ch-1', name: 'alerts', channel_type: 'public' },
  { id: 'ch-2', name: 'secret', channel_type: 'private' },
] as Channel[];

const hostile = '<img src=x onerror="alert(1)"> {{name}}';

function renderPanel(list: Channel[] = channels) {
  render(
    <MemoryRouter>
      <IntegrationsSection workspaceId="ws-1" instanceUrl="http://localhost:8080" channels={list} />
    </MemoryRouter>,
  );
}

describe('IntegrationsSection', () => {
  beforeEach(() => {
    hooks = [];
    createMutate.mockReset().mockImplementation(async () => {
      const created = hook({});
      hooks = [...hooks, created];
      return created;
    });
    deleteMutate.mockReset().mockResolvedValue(undefined);
    enableMutate.mockReset().mockResolvedValue({ hook: hook({}), secrets: null });
    revealMutate.mockReset().mockResolvedValue(secrets({}));
    rotateMutate.mockReset().mockResolvedValue(
      secrets({
        config: { token: 'tok-999' },
        incoming_url: 'http://localhost:8080/api/hooks/incoming/tok-999',
      }),
    );
  });

  it('shows the webhook URL right after creating an incoming hook', async () => {
    renderPanel();

    fireEvent.change(screen.getByLabelText('Incoming webhook name'), { target: { value: 'CI alerts' } });
    fireEvent.change(screen.getByLabelText('Incoming webhook channel'), { target: { value: 'ch-1' } });
    fireEvent.click(screen.getByTestId('incoming-hook-create'));

    await waitFor(() =>
      expect(createMutate).toHaveBeenCalledWith({
        hook_type: 'incoming_webhook',
        name: 'CI alerts',
        config: { channel_id: 'ch-1' },
      }),
    );
    expect(await screen.findByText('http://localhost:8080/api/hooks/incoming/tok-123')).toBeInTheDocument();
  });

  it('keeps credentials hidden until the admin asks to reveal them', async () => {
    hooks = [hook({})];
    renderPanel();

    expect(screen.queryByTestId('hook-secrets')).toBeNull();

    fireEvent.click(screen.getByTestId('hook-reveal'));

    await waitFor(() => expect(revealMutate).toHaveBeenCalledWith('hook-1'));
    expect(await screen.findByTestId('hook-secrets')).toBeInTheDocument();
  });

  it('replaces the shown value after rotating', async () => {
    hooks = [hook({})];
    renderPanel();

    fireEvent.click(screen.getByTestId('hook-reveal'));
    expect(await screen.findByText(/tok-123$/)).toBeInTheDocument();

    fireEvent.click(screen.getByTestId('hook-rotate'));

    await waitFor(() => expect(rotateMutate).toHaveBeenCalledWith('hook-1'));
    expect(await screen.findByText(/tok-999$/)).toBeInTheDocument();
    expect(screen.queryByText(/tok-123$/)).toBeNull();
  });

  it('names the channel an incoming hook posts to, and the URL an outgoing one calls', () => {
    hooks = [
      hook({}),
      hook({
        id: 'hook-2',
        hook_type: 'outgoing_webhook',
        name: 'Deploy bot',
        config: { url: 'https://example.com/hooks/chat' },
      }),
    ];
    renderPanel();

    const rows = screen.getAllByTestId('hook-row');
    expect(within(rows[0]).getByTestId('hook-target')).toHaveTextContent('#alerts');
    expect(within(rows[0]).getByRole('link', { name: '#alerts' })).toHaveAttribute('href', '/app/ws-1/ch-1');
    expect(within(rows[1]).getByTestId('hook-target')).toHaveTextContent('https://example.com/hooks/chat');
  });

  it('asks for confirmation before deleting', async () => {
    hooks = [hook({})];
    renderPanel();

    fireEvent.click(screen.getByTestId('hook-delete'));
    expect(deleteMutate).not.toHaveBeenCalled();

    expect(screen.getByRole('dialog', { name: 'Delete CI alerts?' })).toBeInTheDocument();
    fireEvent.click(screen.getByTestId('hook-delete-dialog-confirm'));
    await waitFor(() => expect(deleteMutate).toHaveBeenCalledWith('hook-1'));
  });

  it('surfaces a failed creation instead of clearing the form', async () => {
    createMutate.mockRejectedValue(new Error('Requires at least Admin role'));
    renderPanel();

    fireEvent.change(screen.getByLabelText('Outgoing webhook name'), { target: { value: 'Deploy bot' } });
    fireEvent.change(screen.getByLabelText('Outgoing webhook URL'), {
      target: { value: 'https://example.com/hooks/chat' },
    });
    fireEvent.click(screen.getByTestId('outgoing-hook-channel-ch-1'));
    fireEvent.click(screen.getByTestId('outgoing-hook-create'));

    expect(await screen.findByText('Requires at least Admin role')).toBeInTheDocument();
    expect(screen.getByLabelText('Outgoing webhook name')).toHaveValue('Deploy bot');
  });

  it('will not create an outgoing webhook until it is scoped to a channel', async () => {
    renderPanel();

    fireEvent.change(screen.getByLabelText('Outgoing webhook name'), { target: { value: 'Deploy bot' } });
    fireEvent.change(screen.getByLabelText('Outgoing webhook URL'), {
      target: { value: 'https://example.com/hooks/chat' },
    });
    expect(screen.getByTestId('outgoing-hook-create')).toBeDisabled();

    fireEvent.click(screen.getByTestId('outgoing-hook-channel-ch-2'));
    fireEvent.click(screen.getByTestId('outgoing-hook-create'));

    await waitFor(() =>
      expect(createMutate).toHaveBeenCalledWith({
        hook_type: 'outgoing_webhook',
        name: 'Deploy bot',
        config: { url: 'https://example.com/hooks/chat', channel_ids: ['ch-2'] },
      }),
    );
  });

  it('says which channels an outgoing webhook forwards', () => {
    hooks = [
      hook({
        id: 'hook-2',
        hook_type: 'outgoing_webhook',
        name: 'Deploy bot',
        config: { url: 'https://example.com/hooks/chat', channel_ids: ['ch-1', 'ch-2'] },
      }),
    ];
    renderPanel();

    expect(screen.getByTestId('hook-scope')).toHaveTextContent('#alerts, #secret');
  });

  it('says why a hook was turned off', () => {
    hooks = [hook({ is_active: false, disabled_reason: "Its creator's account is suspended" })];
    renderPanel();
    expect(screen.getByTestId('hook-inactive')).toHaveTextContent(
      "Disabled — its creator's account is suspended",
    );
  });

  it('turns a stopped hook back on', async () => {
    hooks = [hook({ is_active: false, disabled_reason: 'Its creator is no longer allowed to make it' })];
    renderPanel();

    enableMutate.mockResolvedValue({
      hook: hook({}),
      secrets: secrets({
        config: { token: 'tok-new' },
        incoming_url: 'http://localhost:8080/api/hooks/incoming/tok-new',
      }),
    });

    fireEvent.click(screen.getByTestId('hook-enable'));
    await waitFor(() => expect(enableMutate).toHaveBeenCalledWith('hook-1'));
    expect(await screen.findByTestId('hook-secrets')).toHaveTextContent('tok-new');
  });

  it('shows what an incoming webhook takes as code', () => {
    renderPanel();
    expect(screen.getByText('{"text":"…"}').tagName).toBe('CODE');
  });

  it('offers nothing to turn on for a running hook', () => {
    hooks = [hook({})];
    renderPanel();
    expect(screen.queryByTestId('hook-enable')).toBeNull();
  });
});

describe('IntegrationsSection in Japanese', () => {
  const named = [
    { id: 'ch-1', name: hostile, channel_type: 'public' },
    { id: 'ch-2', name: 'secret', channel_type: 'private' },
  ] as Channel[];

  beforeEach(async () => {
    hooks = [];
    createMutate.mockReset();
    deleteMutate.mockReset().mockResolvedValue(undefined);
    enableMutate.mockReset();
    revealMutate.mockReset();
    rotateMutate.mockReset();
    await i18n.changeLanguage('ja');
  });

  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('labels both kinds of webhook and their forms', () => {
    renderPanel(named);

    expect(screen.getByText('Incoming Webhook')).toBeInTheDocument();
    expect(screen.getByText('Outgoing Webhook')).toBeInTheDocument();
    const code = screen.getByText('{"text":"…"}');
    expect(code.tagName).toBe('CODE');
    expect(code.parentElement).toHaveTextContent(
      '{"text":"…"} を送信できるサービスやツールから、チャンネルに投稿できます。',
    );
    expect(
      screen.getByText(
        '選んだチャンネルに投稿されたメッセージが、下に表示されるシークレットで署名され、指定した URL に POST で送信されます。そのチャンネルのメンバーは全員、この連携が設定されていることを確認できます。',
      ),
    ).toBeInTheDocument();
    expect(screen.getByLabelText('Incoming Webhook の名前')).toHaveAttribute(
      'placeholder',
      '名前（例: CI 通知）',
    );
    expect(screen.getByLabelText('Incoming Webhook の投稿先チャンネル')).toHaveTextContent(
      '投稿先のチャンネル…',
    );
    expect(screen.getByRole('option', { name: `#${hostile}` })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Incoming Webhook を作成' })).toBeInTheDocument();
    expect(screen.getByLabelText('Outgoing Webhook の名前')).toHaveAttribute(
      'placeholder',
      '名前（例: デプロイボット）',
    );
    expect(screen.getByLabelText('Outgoing Webhook の送信先 URL')).toBeInTheDocument();
    expect(screen.getByLabelText(`#${hostile}`)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Outgoing Webhook を作成' })).toBeInTheDocument();
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  it('names each webhook, its channels and its buttons as text', () => {
    hooks = [
      hook({ name: hostile }),
      hook({
        id: 'hook-2',
        hook_type: 'outgoing_webhook',
        name: 'Deploy bot',
        config: { url: 'https://example.com/hooks/chat', channel_ids: ['ch-1', 'ch-2', 'ch-gone'] },
      }),
    ];
    renderPanel(named);

    const rows = screen.getAllByTestId('hook-row');
    expect(within(rows[0]).getByTestId('hook-target')).toHaveTextContent(`#${hostile}`);
    expect(within(rows[1]).getByTestId('hook-scope').textContent).toBe(
      `転送先: #${hostile}、#secret、#不明なチャンネル`,
    );
    expect(within(rows[0]).getByRole('button', { name: `${hostile} の認証情報を表示` })).toHaveAttribute(
      'data-tooltip',
      '表示',
    );
    expect(within(rows[0]).getByRole('button', { name: `${hostile} の認証情報を再発行` })).toHaveAttribute(
      'data-tooltip',
      '再発行（古い値はすぐに使えなくなります）',
    );
    expect(within(rows[0]).getByRole('button', { name: `${hostile} を削除` })).toHaveAttribute(
      'data-tooltip',
      '削除',
    );
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  it('says a webhook stopped, with and without a reason, and turns it on', async () => {
    hooks = [
      hook({ is_active: false, disabled_reason: "Its creator's account is suspended" }),
      hook({ id: 'hook-2', is_active: false, disabled_reason: null, config: {} }),
    ];
    enableMutate.mockResolvedValue({ hook: hook({}), secrets: null });
    renderPanel(named);

    const rows = screen.getAllByTestId('hook-row');
    expect(within(rows[0]).getByTestId('hook-inactive')).toHaveTextContent(
      "停止中: its creator's account is suspended",
    );
    expect(within(rows[1]).getByTestId('hook-inactive').textContent).toBe('停止中');
    expect(within(rows[1]).getByTestId('hook-target')).toHaveTextContent('#不明なチャンネル');
    fireEvent.click(within(rows[0]).getByRole('button', { name: 'オンにする' }));
    await waitFor(() => expect(enableMutate).toHaveBeenCalledWith('hook-1'));
  });

  it('shows the URL and the signing secret under Japanese labels', async () => {
    hooks = [
      hook({}),
      hook({
        id: 'hook-2',
        hook_type: 'outgoing_webhook',
        config: { url: 'https://example.com/hooks/chat', channel_ids: ['ch-1'] },
      }),
    ];
    revealMutate.mockImplementation(async (id: string) =>
      id === 'hook-1'
        ? secrets({})
        : secrets({
            hook_id: 'hook-2',
            hook_type: 'outgoing_webhook',
            config: { secret: 's3cret' },
            incoming_url: null,
          }),
    );
    renderPanel(named);

    const rows = screen.getAllByTestId('hook-row');
    fireEvent.click(within(rows[0]).getByTestId('hook-reveal'));
    expect(await within(rows[0]).findByText('Webhook の URL')).toBeInTheDocument();
    fireEvent.click(within(rows[1]).getByTestId('hook-reveal'));
    expect(await within(rows[1]).findByText('署名シークレット')).toBeInTheDocument();
  });

  it('asks before deleting either kind, naming it as text', () => {
    hooks = [
      hook({ name: hostile }),
      hook({
        id: 'hook-2',
        hook_type: 'outgoing_webhook',
        name: hostile,
        config: { url: 'https://x.example' },
      }),
    ];
    renderPanel(named);

    const rows = screen.getAllByTestId('hook-row');
    fireEvent.click(within(rows[0]).getByTestId('hook-delete'));
    const incoming = screen.getByRole('dialog', { name: `${hostile} を削除しますか?` });
    expect(within(incoming).getByText('この URL への投稿は、今後すべて拒否されます。')).toBeInTheDocument();
    expect(within(incoming).getByRole('button', { name: '連携を削除' })).toBeInTheDocument();
    fireEvent.click(within(incoming).getByRole('button', { name: 'キャンセル' }));

    fireEvent.click(within(rows[1]).getByTestId('hook-delete'));
    const outgoing = screen.getByRole('dialog', { name: `${hostile} を削除しますか?` });
    expect(
      within(outgoing).getByText('この URL には、メッセージが送信されなくなります。'),
    ).toBeInTheDocument();
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  it('says what failed when the server gives no message', async () => {
    hooks = [hook({}), hook({ id: 'hook-2', is_active: false })];
    createMutate.mockRejectedValue({});
    revealMutate.mockRejectedValue({});
    rotateMutate.mockRejectedValue({});
    enableMutate.mockRejectedValue({});
    renderPanel(named);

    fireEvent.change(screen.getByLabelText('Incoming Webhook の名前'), { target: { value: 'CI' } });
    fireEvent.change(screen.getByLabelText('Incoming Webhook の投稿先チャンネル'), {
      target: { value: 'ch-2' },
    });
    fireEvent.click(screen.getByTestId('incoming-hook-create'));
    expect(await screen.findByText('Webhook を作成できませんでした。')).toBeInTheDocument();

    const rows = screen.getAllByTestId('hook-row');
    fireEvent.click(within(rows[0]).getByTestId('hook-reveal'));
    expect(await screen.findByText('認証情報を表示できませんでした。')).toBeInTheDocument();
    fireEvent.click(within(rows[0]).getByTestId('hook-rotate'));
    expect(await screen.findByText('認証情報を再発行できませんでした。')).toBeInTheDocument();
    fireEvent.click(within(rows[1]).getByTestId('hook-enable'));
    expect(await screen.findByText('Webhook をオンにできませんでした。')).toBeInTheDocument();
  });

  it('says when there is no channel to forward', () => {
    renderPanel([]);
    expect(screen.getByText('転送できるチャンネルがありません。')).toBeInTheDocument();
  });
});
