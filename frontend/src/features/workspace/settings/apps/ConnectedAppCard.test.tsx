import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import i18n from '@/lib/i18n';
import { ConnectedAppCard } from './ConnectedAppCard';
import type { AppStatus } from '@/features/workspace/hooks/useApps';

const hostile = '<img src=x onerror="alert(1)"> {{name}}';

const configureMutate = vi.fn();

vi.mock('@/features/workspace/hooks/useApps', () => ({
  useAppActions: () => ({
    configure: { mutateAsync: configureMutate, isPending: false },
    remove: { mutate: vi.fn(), isPending: false },
    connect: { mutate: vi.fn(), isPending: false },
    disconnect: { mutate: vi.fn(), isPending: false },
    prompts: { mutate: vi.fn(), isPending: false },
  }),
}));

const PERMISSIONS = [
  { key: 'jira.0', text: 'See the issues and projects each person who connects can see' },
  { key: 'jira.1', text: 'Stay connected until that person disconnects' },
];

const status = (over: Partial<AppStatus> = {}): AppStatus => ({
  provider: 'jira',
  name: 'Jira Cloud',
  summary: 'Issues',
  configured: false,
  enabled: false,
  prompts_off: false,
  client_id: null,
  connected_account: null,
  console_url: 'https://developer.atlassian.com/console/myapps/',
  redirect_url: 'https://chat.test/api/apps/jira/callback',
  scopes: ['read:jira-work'],
  setup_steps: ['Make an app'],
  ...over,
});

function renderCard(over: Partial<AppStatus> = {}) {
  render(<ConnectedAppCard workspaceId="ws-1" app={status(over)} isAdmin permissions={PERMISSIONS} />);
}

describe('ConnectedAppCard', () => {
  beforeEach(() => {
    configureMutate.mockReset().mockResolvedValue({});
  });

  it('keeps the setup of an app nobody has set up folded until asked for', () => {
    renderCard();
    expect(screen.getByText('Not set up')).toBeInTheDocument();
    expect(screen.queryByTestId('app-setup-form')).not.toBeInTheDocument();

    fireEvent.click(screen.getByText('Set up'));
    expect(screen.getByTestId('app-setup-form')).toBeInTheDocument();
  });

  it('installs only after an admin agrees to what it can see', async () => {
    renderCard();
    fireEvent.click(screen.getByText('Set up'));
    fireEvent.change(screen.getByLabelText(/Client ID/), { target: { value: 'client-1' } });
    fireEvent.change(screen.getByLabelText(/Secret/), { target: { value: 'shh' } });
    fireEvent.submit(screen.getByTestId('app-setup-form'));

    expect(configureMutate).not.toHaveBeenCalled();
    const consent = await screen.findByTestId('app-consent-permissions');
    expect(consent).toHaveTextContent('See the issues and projects');
    expect(consent).toHaveTextContent('Stay connected until that person disconnects');

    fireEvent.click(screen.getByTestId('app-consent-confirm'));
    await waitFor(() =>
      expect(configureMutate.mock.calls[0][0]).toEqual({
        provider: 'jira',
        clientId: 'client-1',
        clientSecret: 'shh',
        enabled: true,
      }),
    );
  });

  it('saves a change to an installed app without asking again', () => {
    renderCard({ configured: true, enabled: true, client_id: 'client-1' });
    fireEvent.click(screen.getByText('Edit setup'));
    fireEvent.submit(screen.getByTestId('app-setup-form'));

    expect(screen.queryByTestId('app-consent-permissions')).not.toBeInTheDocument();
    expect(configureMutate).toHaveBeenCalled();
  });
});

describe('ConnectedAppCard in Japanese', () => {
  beforeEach(async () => {
    configureMutate.mockReset().mockResolvedValue({});
    await i18n.changeLanguage('ja');
  });

  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('walks an admin through setting it up, and names the app as text', async () => {
    renderCard({ name: hostile });

    expect(screen.getByText('未設定')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '設定する' }));
    expect(screen.getByRole('button', { name: '設定を閉じる' })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: '開発者コンソールを開く' })).toBeInTheDocument();
    expect(screen.getByText('コールバック URL')).toBeInTheDocument();
    expect(screen.getByText('スコープ')).toBeInTheDocument();
    expect(screen.getByLabelText('このワークスペースでオンにする')).toBeChecked();
    fireEvent.change(screen.getByLabelText(/クライアント ID/), { target: { value: 'client-1' } });
    fireEvent.change(screen.getByLabelText(/シークレット/), { target: { value: 'shh' } });
    fireEvent.click(screen.getByRole('button', { name: '保存' }));

    expect(await screen.findByText(`${hostile} をインストールしますか?`)).toBeInTheDocument();
    expect(
      screen.getByText(
        '同意するまで、誰もアカウントを接続できません。接続した人ごとに、このアプリは次のことができるようになります:',
      ),
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '同意してインストール' })).toBeInTheDocument();
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  it('offers to reuse a sibling app, and to remove an installed one', () => {
    render(
      <ConnectedAppCard
        workspaceId="ws-1"
        app={status({ provider: 'confluence', name: hostile, configured: true, enabled: true })}
        reusable={status({ name: 'Jira Cloud', configured: true, client_id: 'client-1' })}
        isAdmin
        permissions={PERMISSIONS}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: '設定を変更' }));
    expect(screen.getByLabelText('Jira Cloud のアプリを使う')).toBeInTheDocument();
    expect(screen.getByText(`先に、そのアプリに ${hostile} の API を追加してください。`)).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'アプリを削除' }));
    expect(screen.getByRole('button', { name: '削除して、全員の接続を解除' })).toBeInTheDocument();
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  it('tells a member how to connect, or why they cannot', () => {
    render(
      <ConnectedAppCard
        workspaceId="ws-1"
        app={status({ configured: true, enabled: true, prompts_off: true })}
        isAdmin={false}
        permissions={PERMISSIONS}
      />,
    );
    expect(screen.getByRole('button', { name: '自分のアカウントを接続' })).toBeInTheDocument();
    expect(screen.getByText('接続の案内: オフ')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'オンにする' })).toBeInTheDocument();
  });

  it('shows a connected account, and the notes for a member', () => {
    const { unmount } = render(
      <ConnectedAppCard
        workspaceId="ws-1"
        app={status({ configured: true, enabled: true, connected_account: hostile })}
        isAdmin={false}
        permissions={PERMISSIONS}
      />,
    );
    expect(screen.getByText('接続済み')).toBeInTheDocument();
    expect(screen.getByText(hostile)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '接続を解除' })).toBeInTheDocument();
    expect(document.querySelector('[onerror]')).toBeNull();
    unmount();

    const { unmount: second } = render(
      <ConnectedAppCard
        workspaceId="ws-1"
        app={status({ configured: true, enabled: false })}
        isAdmin={false}
        permissions={PERMISSIONS}
      />,
    );
    expect(screen.getByText('オフ')).toBeInTheDocument();
    expect(screen.getByText('管理者がこのアプリをオフにしています。')).toBeInTheDocument();
    second();

    render(<ConnectedAppCard workspaceId="ws-1" app={status()} isAdmin={false} permissions={[]} />);
    expect(
      screen.getByText('ワークスペースの管理者に、このアプリの設定を頼んでください。'),
    ).toBeInTheDocument();
  });
});
