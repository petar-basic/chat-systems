import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import type { ReactElement } from 'react';
import { fireEvent, render as rtlRender, screen, waitFor, within } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ApiError } from '@/lib/errors';
import { useWorkspaceStore, type Workspace } from '@/stores/workspace';
import i18n from '@/lib/i18n';
import { BlockMessage } from './BlockMessage';
import { everyElement, githubActions, grafana, jenkins, laidOut, sentry } from './fixtures';

vi.mock('@/features/instance/hooks/useAuth', () => ({
  useCurrentUser: () => ({ data: { id: 'u-self' } }),
}));

const calls = { get: vi.fn(), post: vi.fn() };
vi.mock('@/shared/hooks/useCurrentApi', async () => {
  const { typedApiMock } = await import('@/test/typedApiMock');
  const api = () =>
    typedApiMock({
      get: (...args) => calls.get(...args),
      post: (...args) => calls.post(...args),
    });
  return { useCurrentApi: api, getApiForInstance: api };
});

function render(ui: ReactElement) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return rtlRender(<QueryClientProvider client={client}>{ui}</QueryClientProvider>);
}

const DEPLOYER = {
  app_id: 'app-1',
  bot_user_id: 'u-hook',
  name: 'Deployer',
  description: null,
  icon_url: null,
  interactive: true,
  home: false,
  hook_ids: ['hook-1'],
  shortcuts: [],
};

function appsAre(apps: (typeof DEPLOYER)[]) {
  calls.get.mockImplementation(async (path: string) =>
    path === '/workspaces/{ws_id}/active-apps' ? { data: apps } : [],
  );
}

const deploy = (over: Partial<ReturnType<typeof laidOut>> = {}) => ({
  ...laidOut('', {
    blocks: [
      { type: 'section', block_id: 'what', text: { type: 'mrkdwn', text: 'Deploy **api** to production?' } },
      {
        type: 'actions',
        block_id: 'deploy',
        elements: [
          {
            type: 'button',
            action_id: 'approve',
            value: 'v2',
            text: { type: 'plain_text', text: 'Approve' },
          },
          {
            type: 'static_select',
            action_id: 'region',
            placeholder: { type: 'plain_text', text: 'Region' },
            options: [
              { text: { type: 'plain_text', text: 'Europe' }, value: 'eu' },
              { text: { type: 'plain_text', text: 'America' }, value: 'us' },
            ],
          },
        ],
      },
    ],
  }),
  ...over,
});

beforeEach(() => {
  calls.get.mockReset();
  calls.post.mockReset();
  appsAre([]);
  calls.post.mockResolvedValue({ status: 'ok' });
  useWorkspaceStore.setState({
    currentWorkspace: { id: 'ws-1', instanceUrl: 'http://localhost:8080' } as Workspace,
  });
});

const INERT = 'This app has not set up interactivity';

describe('BlockMessage', () => {
  it('shows a GitHub Actions run as its header, fields and a button that opens the run', () => {
    render(<BlockMessage message={githubActions} />);
    expect(screen.getByTestId('block-header')).toHaveTextContent('✅ Build passed');
    const fields = screen.getByTestId('block-fields');
    expect(fields.children).toHaveLength(4);
    expect(within(fields).getByRole('link', { name: 'acme/api' })).toHaveAttribute(
      'href',
      'https://github.com/acme/api',
    );
    expect(fields.children[1].querySelector('br')).toBeInTheDocument();
    expect(screen.getByTestId('block-divider')).toBeInTheDocument();
    const run = screen.getByRole('link', { name: 'View run' });
    expect(run).toHaveAttribute('href', 'https://github.com/acme/api/actions/runs/9201');
    expect(run).toHaveAttribute('target', '_blank');
    expect(run).toHaveAttribute('rel', 'noopener noreferrer nofollow');
    expect(screen.queryByText('GitHub Action build result: success')).not.toBeInTheDocument();
  });

  it('shows a Grafana alert as a coloured bar with its title link and time', () => {
    render(<BlockMessage message={grafana} />);
    const bar = screen.getByTestId('legacy-attachment-bar');
    expect(bar).toHaveAttribute('data-colour', '#d63232');
    expect(screen.getByRole('link', { name: '[FIRING:1] High CPU (api-1)' })).toHaveAttribute(
      'href',
      'https://grafana.example.com/alerting/list',
    );
    expect(within(bar).getByText('Firing').closest('strong')).not.toBeNull();
    expect(within(bar).getAllByRole('listitem')).toHaveLength(2);
    const footer = screen.getByTestId('legacy-footer');
    expect(footer).toHaveTextContent('Grafana v10.4.1');
    expect(footer.querySelector('time')).toHaveAttribute('datetime', '2023-11-14T22:13:20.000Z');
    expect(footer.querySelector('time')).toHaveTextContent('2023');
    expect(screen.getAllByTestId('legacy-attachment')).toHaveLength(1);
  });

  it('shows a Sentry issue with short fields side by side and a linked footer', () => {
    render(<BlockMessage message={sentry} />);
    const fields = screen.getByTestId('legacy-fields');
    expect(within(fields).getByText('Environment')).toBeInTheDocument();
    expect(within(fields).getByText('production')).toBeInTheDocument();
    expect(fields.querySelectorAll('.col-span-2')).toHaveLength(0);
    expect(screen.getByRole('link', { name: 'BACKEND-3QZ' })).toHaveAttribute(
      'href',
      'https://sentry.io/organizations/acme/issues/4512/',
    );
  });

  it('shows a Jenkins build with Slack’s named colour and no repeat of its flattened text', () => {
    render(<BlockMessage message={jenkins} />);
    expect(screen.getByTestId('legacy-attachment-bar')).toHaveAttribute('data-colour', '#2eb886');
    expect(screen.getAllByRole('link', { name: 'Open' })).toHaveLength(1);
  });

  it('shows the text above attachments when the app sent its own', () => {
    render(
      <BlockMessage
        message={laidOut('Nightly finished', {
          legacy_attachments: [
            { color: 'javascript:alert(1)', text: 'three failed', fields: [], blocks: [] },
          ],
        })}
      />,
    );
    expect(screen.getByText('Nightly finished')).toBeInTheDocument();
    const bar = screen.getByTestId('legacy-attachment-bar');
    expect(bar).not.toHaveAttribute('data-colour');
    expect(bar.getAttribute('style')).toBeNull();
  });

  it('shows every element, and every one but a link button is inert with a reason', () => {
    render(<BlockMessage message={everyElement} />);
    expect(screen.getByAltText('Release logo')).toHaveAttribute('referrerpolicy', 'no-referrer');
    const actions = screen.getByTestId('block-actions');
    const controls = within(actions).getAllByRole('button');
    expect(controls).toHaveLength(7);
    for (const control of controls) {
      expect(control).toHaveAttribute('aria-disabled', 'true');
      expect(control).toHaveAttribute('data-tooltip', INERT);
    }
    expect(within(actions).getByRole('button', { name: 'Approve' })).toHaveClass('bg-emerald-700');
    expect(within(actions).getByRole('button', { name: 'Reject' })).toHaveClass('bg-red-700');
    const selects = within(actions).getAllByTestId('block-select');
    expect(selects.map((select) => select.textContent)).toEqual([
      'Roll out to',
      'Reviewer',
      'Select a conversation',
      '2026-11-20',
    ]);
    expect(within(actions).getByRole('button', { name: 'More options' })).toBeInTheDocument();
    const context = screen.getByTestId('block-context');
    expect(context).toHaveTextContent('Asked by Maya2 minutes ago');
    expect(within(context).getByAltText('Maya')).toBeInTheDocument();
    const image = screen.getByTestId('block-image');
    expect(image).toHaveTextContent('Error rate, last hour');
    expect(within(image).getByAltText('Error rate')).toHaveAttribute(
      'src',
      'https://cdn.example.com/chart.png',
    );
  });

  it('never links a button whose url is not the web', () => {
    render(
      <BlockMessage
        message={laidOut('', {
          blocks: [
            {
              type: 'actions',
              elements: [
                { type: 'button', text: { type: 'plain_text', text: 'Run' }, url: 'javascript:alert(1)' },
              ],
            },
          ],
        })}
      />,
    );
    expect(screen.queryByRole('link')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Run' })).toHaveAttribute('aria-disabled', 'true');
  });

  it('keeps a plain text block as text, not markup', () => {
    render(
      <BlockMessage
        message={laidOut('', {
          blocks: [{ type: 'section', text: { type: 'plain_text', text: '**not bold**' } }],
        })}
      />,
    );
    expect(screen.getByText('**not bold**')).toBeInTheDocument();
    expect(document.querySelector('strong')).toBeNull();
  });
});

describe('BlockMessage from an app with interactivity on', () => {
  const live = (name: string) =>
    waitFor(() => expect(screen.getByRole('button', { name })).not.toHaveAttribute('aria-disabled'));

  it('sends a button to the app by its block and action ids', async () => {
    appsAre([DEPLOYER]);
    render(<BlockMessage message={deploy()} />);
    await live('Approve');
    fireEvent.click(screen.getByRole('button', { name: 'Approve' }));
    await waitFor(() => expect(calls.post).toHaveBeenCalledOnce());
    expect(calls.post).toHaveBeenCalledWith('/messages/{msg_id}/actions', {
      block_id: 'deploy',
      action_id: 'approve',
      value: undefined,
    });
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('sends the option somebody picks', async () => {
    appsAre([DEPLOYER]);
    render(<BlockMessage message={deploy()} />);
    await live('Approve');
    fireEvent.change(screen.getByLabelText('Region'), { target: { value: 'us' } });
    await waitFor(() => expect(calls.post).toHaveBeenCalledOnce());
    expect(calls.post.mock.calls[0][1]).toEqual({ block_id: 'deploy', action_id: 'region', value: 'us' });
  });

  it('stays drawn but dead while the app has interactivity off', async () => {
    appsAre([{ ...DEPLOYER, interactive: false }]);
    render(<BlockMessage message={deploy()} />);
    await waitFor(() => expect(calls.get).toHaveBeenCalled());
    expect(screen.getByRole('button', { name: 'Approve' })).toHaveAttribute('aria-disabled', 'true');
  });

  it('says so under the block when the app does not answer', async () => {
    appsAre([DEPLOYER]);
    calls.post.mockRejectedValue(new ApiError(503, 'The app did not respond'));
    render(<BlockMessage message={deploy()} />);
    await live('Approve');
    fireEvent.click(screen.getByRole('button', { name: 'Approve' }));
    const failure = await screen.findByTestId('block-failure');
    expect(failure).toHaveTextContent('The app did not respond');
    expect(failure).toHaveAttribute('role', 'alert');
    expect(screen.getByRole('button', { name: 'Approve' })).not.toBeDisabled();
  });

  it('is live for an answer the app posted through its command', async () => {
    appsAre([DEPLOYER]);
    const answered = deploy({ user_id: 'u-ana' });
    answered.metadata = {
      ...answered.metadata,
      bot: { name: 'Deployer', icon_url: null, hook_id: 'hook-1' },
    };
    render(<BlockMessage message={answered} />);
    await live('Approve');
  });

  it('stays inert for a message the app did not post', async () => {
    appsAre([DEPLOYER]);
    render(<BlockMessage message={deploy({ user_id: 'u-ana' })} />);
    await waitFor(() => expect(calls.get).toHaveBeenCalled());
    const approve = screen.getByRole('button', { name: 'Approve' });
    expect(approve).toHaveAttribute('aria-disabled', 'true');
    expect(approve).toHaveAttribute('data-tooltip', INERT);
    fireEvent.click(approve);
    expect(calls.post).not.toHaveBeenCalled();
  });
});

describe('BlockMessage in Japanese', () => {
  beforeEach(async () => {
    await i18n.changeLanguage('ja');
  });

  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  const live = (name: string) =>
    waitFor(() => expect(screen.getByRole('button', { name })).not.toHaveAttribute('aria-disabled'));

  it('says why an element is dead, and what to pick where the app named nothing', () => {
    render(
      <BlockMessage
        message={laidOut('', {
          blocks: [
            {
              type: 'actions',
              block_id: 'pick',
              elements: [
                { type: 'button', action_id: 'go', text: { type: 'plain_text', text: 'Go' } },
                {
                  type: 'static_select',
                  action_id: 'one',
                  options: [{ text: { type: 'plain_text', text: 'A' }, value: 'a' }],
                },
                { type: 'users_select', action_id: 'who' },
                { type: 'conversations_select', action_id: 'where' },
                { type: 'datepicker', action_id: 'when' },
                {
                  type: 'overflow',
                  action_id: 'more',
                  options: [{ text: { type: 'plain_text', text: 'B' }, value: 'b' }],
                },
              ],
            },
          ],
        })}
      />,
    );

    const actions = screen.getByTestId('block-actions');
    expect(within(actions).getByRole('button', { name: 'Go' })).toHaveAttribute(
      'data-tooltip',
      'このアプリにはインタラクティブ機能が設定されていません',
    );
    expect(
      within(actions)
        .getAllByTestId('block-select')
        .map((select) => select.textContent),
    ).toEqual(['選択してください', 'メンバーを選択', '会話を選択', '日付を選択']);
    expect(within(actions).getByRole('button', { name: 'その他のオプション' })).toBeInTheDocument();
  });

  it('says so when the app does not answer, or will not take it', async () => {
    appsAre([DEPLOYER]);
    calls.post.mockRejectedValueOnce(new ApiError(503, 'The app did not respond')).mockRejectedValueOnce({});
    render(<BlockMessage message={deploy()} />);
    await live('Approve');

    fireEvent.click(screen.getByRole('button', { name: 'Approve' }));
    expect(await screen.findByTestId('block-failure')).toHaveTextContent('アプリから応答がありませんでした');
    fireEvent.click(screen.getByRole('button', { name: 'Approve' }));
    await waitFor(() =>
      expect(screen.getByTestId('block-failure')).toHaveTextContent('アプリがこの操作を受け付けませんでした'),
    );
  });
});
