import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import i18n from '@/lib/i18n';
import { formatDateTime } from '@/lib/datetime';
import { render, screen, fireEvent, waitFor, within } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ChannelDetailsModal, type ChannelDetailsTab } from './ChannelDetailsModal';
import type { Channel } from '@/stores/workspace';
import type { ChannelRetention } from '@/features/workspace/hooks/useRetention';
import type { ChannelIntegrations } from '../hooks/useChannelIntegrations';
import type { ChannelAnalytics } from '../hooks/useChannelAnalytics';
import type { AppRef } from '@/features/workspace';
import type { WorkspaceRole } from '@/stores/workspace';

const updateChannel = vi.fn();
const setRetention = vi.fn();
const createHook = vi.fn();
const revealHook = vi.fn();
const rotateHook = vi.fn();
const deleteHook = vi.fn();
const setPrivacy = vi.fn();
let role: WorkspaceRole | null = 'member';
let contents: { private: boolean; messages: number; files: number } | undefined;
let retention: ChannelRetention;
let integrations: ChannelIntegrations;
let analytics: ChannelAnalytics;
let memberName = 'Ana';

vi.mock('../hooks/useChannels', () => ({
  useUpdateChannel: () => ({ mutateAsync: updateChannel, isPending: false }),
  useArchiveChannel: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useChannelPrivacy: () => ({ data: contents }),
  useSetChannelPrivacy: () => ({ mutateAsync: setPrivacy, isPending: false }),
}));
vi.mock('@/features/workspace/hooks/useCurrentWorkspaceRole', () => ({
  useCurrentWorkspaceRole: () => ({ role, isResolved: true }),
}));
vi.mock('@/features/workspace/hooks/useRetention', () => ({
  useChannelRetention: () => ({ data: retention }),
  useSetChannelRetention: () => ({ mutateAsync: setRetention, isPending: false }),
}));
vi.mock('@/features/workspace/hooks/useHooks', () => ({
  useCreateHook: () => ({ mutateAsync: createHook, isPending: false }),
  useRevealHook: () => ({ mutateAsync: revealHook, isPending: false }),
  useRotateHook: () => ({ mutateAsync: rotateHook, isPending: false }),
  useDeleteHook: () => ({ mutateAsync: deleteHook, isPending: false }),
  useHookedChannels: () => ({ data: new Set() }),
}));
vi.mock('@/features/workspace/hooks/useWorkspaces', () => ({
  useWorkspaceMembers: () => ({ data: [{ user_id: 'u-ana', display_name: memberName }] }),
  useWorkspaceChannels: () => ({ data: [] }),
}));
vi.mock('../hooks/useChannelIntegrations', () => ({
  useChannelIntegrations: () => ({ isLoading: false, data: integrations }),
}));
vi.mock('../hooks/useChannelAnalytics', () => ({
  useChannelAnalytics: () => ({ isLoading: false, isError: false, data: analytics }),
}));

const channel = {
  id: 'ch-1',
  workspace_id: 'ws-1',
  name: 'deploys',
  topic: 'Shipping',
  description: '',
  settings: {},
} as unknown as Channel;

function renderModal({
  of = channel,
  canModerate = true,
  canEditAbout = false,
  initialTab = 'about',
  onOpenWorkflow,
  onOpenApp,
}: {
  of?: Channel;
  canModerate?: boolean;
  canEditAbout?: boolean;
  initialTab?: ChannelDetailsTab;
  onOpenWorkflow?: (id: string) => void;
  onOpenApp?: (app: AppRef) => void;
} = {}) {
  const onClose = vi.fn();
  render(
    <QueryClientProvider client={new QueryClient()}>
      <ChannelDetailsModal
        channel={of}
        canModerate={canModerate}
        canEditAbout={canEditAbout}
        initialTab={initialTab}
        onOpenWorkflow={onOpenWorkflow}
        onOpenApp={onOpenApp}
        onClose={onClose}
      />
    </QueryClientProvider>,
  );
  return onClose;
}

const tabs = () => screen.getAllByRole('tab').map((tab) => tab.textContent);
const save = () => fireEvent.click(screen.getByTestId('channel-settings-save'));

beforeEach(() => {
  memberName = 'Ana';
  role = 'member';
  contents = { private: true, messages: 12, files: 1 };
  setPrivacy.mockReset().mockResolvedValue({});
  updateChannel.mockReset().mockResolvedValue({});
  setRetention.mockReset().mockResolvedValue({});
  createHook.mockReset().mockResolvedValue({ id: 'h-new' });
  revealHook.mockReset().mockResolvedValue({ incoming_url: 'https://chat.test/api/hooks/incoming/first' });
  rotateHook.mockReset().mockResolvedValue({ incoming_url: 'https://chat.test/api/hooks/incoming/second' });
  deleteHook.mockReset().mockResolvedValue({});
  analytics = {
    from: '2026-10-07',
    to: '2026-11-05',
    channel: {
      channel_id: 'ch-1',
      name: 'deploys',
      channel_type: 'public',
      is_archived: false,
      created_at: '2026-01-15T10:00:00Z',
      members: 6,
      guests: 0,
      messages: 1234,
      posters: 2,
      viewers: 5,
      reactions: 0,
      last_message_at: '2026-11-04T08:30:00Z',
    },
  };
  retention = { overrides_allowed: true, workspace_message_days: 365, channel: null };
  integrations = {
    incoming_webhooks: [
      {
        id: 'h-ci',
        name: 'CI',
        description: null,
        created_by: 'u-ana',
        created_at: '2026-10-20T10:00:00Z',
      },
    ],
    outgoing_webhooks: [{ id: 'h-out', name: 'Mirror', description: 'to the archive' }],
    slash_commands: [{ id: 'h-cmd', name: 'Deployer', command: 'deploy' }],
    workflows: [{ id: 'w-1', name: 'Standup', enabled: true, created_by: 'u-ana' }],
    can_add_webhook: true,
  };
});

describe('ChannelDetailsModal', () => {
  it('gives a moderator About, Settings, Activity and Integrations, and everyone else About and Integrations', () => {
    renderModal();
    expect(tabs()).toEqual(['About', 'Settings', 'Activity', 'Integrations']);
    expect(screen.getByTestId('channel-about-name')).toHaveValue('deploys');
  });

  it('shows someone who may not post here what the channel is about without letting them edit it', () => {
    renderModal({ canModerate: false, initialTab: 'settings' });
    expect(tabs()).toEqual(['About', 'Integrations']);
    expect(screen.getByTestId('channel-about')).toHaveTextContent('Shipping');
    expect(screen.queryByTestId('channel-about-name')).not.toBeInTheDocument();
    expect(screen.queryByTestId('channel-about-topic')).not.toBeInTheDocument();
  });

  it('lets a member who posts here change the topic and description, never the name or the archive', async () => {
    const onClose = renderModal({ canModerate: false, canEditAbout: true });
    expect(tabs()).toEqual(['About', 'Integrations']);
    expect(screen.queryByTestId('channel-about-name')).not.toBeInTheDocument();
    expect(screen.queryByTestId('channel-about-archive')).not.toBeInTheDocument();
    fireEvent.change(screen.getByTestId('channel-about-topic'), { target: { value: 'Releases' } });
    fireEvent.change(screen.getByTestId('channel-about-description'), { target: { value: 'Where we ship' } });
    fireEvent.click(screen.getByTestId('channel-about-save'));
    await waitFor(() =>
      expect(updateChannel).toHaveBeenCalledWith({ topic: 'Releases', description: 'Where we ship' }),
    );
    expect(onClose).toHaveBeenCalled();
  });

  it('saves the name, topic and description from About', async () => {
    const onClose = renderModal();
    fireEvent.change(screen.getByTestId('channel-about-topic'), { target: { value: 'Releases' } });
    fireEvent.click(screen.getByTestId('channel-about-save'));
    await waitFor(() =>
      expect(updateChannel).toHaveBeenCalledWith({ name: 'deploys', topic: 'Releases', description: '' }),
    );
    expect(onClose).toHaveBeenCalled();
  });
});

describe('ChannelDetailsModal activity', () => {
  it('counts the last 30 days of the channel for its moderator', () => {
    renderModal({ initialTab: 'activity' });
    expect(screen.getByTestId('channel-activity')).toHaveTextContent('Oct 7 – Nov 5');
    expect(screen.getByTestId('channel-activity-messages')).toHaveTextContent('1,234');
    expect(screen.getByTestId('channel-activity-posters')).toHaveTextContent('2of 6 members');
    expect(screen.getByTestId('channel-activity-viewers')).toHaveTextContent('5of 6 members');
    expect(screen.getByTestId('channel-activity-last')).toHaveTextContent('Nov 4');
  });

  it('keeps the numbers from a member who does not moderate', () => {
    renderModal({ canModerate: false, initialTab: 'activity' });
    expect(screen.queryByTestId('channel-activity')).not.toBeInTheDocument();
    expect(screen.getByTestId('channel-about')).toBeInTheDocument();
  });
});

describe('ChannelDetailsModal settings', () => {
  it('shows the workspace policy while channels may not have their own', () => {
    retention = {
      overrides_allowed: false,
      workspace_message_days: 90,
      channel: { message_days: 7, updated_by: null, updated_at: '' },
    };
    renderModal({ initialTab: 'settings' });
    expect(screen.getByTestId('channel-retention-workspace')).toHaveTextContent('kept for 90 days');
    expect(screen.queryByTestId('channel-retention-mode')).not.toBeInTheDocument();
  });

  it('saves who may post and a period of its own', async () => {
    const onClose = renderModal({ initialTab: 'settings' });
    expect(screen.getByRole('option', { name: 'Workspace default (365 days)' })).toBeInTheDocument();

    fireEvent.click(screen.getByTestId('channel-settings-announcement'));
    fireEvent.change(screen.getByTestId('channel-retention-mode'), { target: { value: 'days' } });
    fireEvent.change(screen.getByTestId('channel-retention-days'), { target: { value: '14' } });
    save();

    await waitFor(() => expect(setRetention).toHaveBeenCalledWith(14));
    expect(updateChannel).toHaveBeenCalledWith({ post_policy: 'moderators' });
    expect(onClose).toHaveBeenCalled();
  });

  it('turns @channel, @here and @everyone off for the channel, and sends the switch only when it moved', async () => {
    renderModal({ initialTab: 'settings' });
    expect(screen.getByTestId('channel-settings-broadcast-mentions')).toBeChecked();

    fireEvent.click(screen.getByTestId('channel-settings-broadcast-mentions'));
    save();

    await waitFor(() =>
      expect(updateChannel).toHaveBeenCalledWith({
        post_policy: 'everyone',
        allow_broadcast_mentions: false,
      }),
    );
  });

  it('keeps forever, or goes back to the workspace', async () => {
    retention = { ...retention, channel: { message_days: 30, updated_by: null, updated_at: '' } };
    renderModal({ initialTab: 'settings' });
    expect(screen.getByTestId('channel-retention-days')).toHaveValue(30);

    fireEvent.change(screen.getByTestId('channel-retention-mode'), { target: { value: 'forever' } });
    save();
    await waitFor(() => expect(setRetention).toHaveBeenLastCalledWith(null));

    fireEvent.change(screen.getByTestId('channel-retention-mode'), { target: { value: 'workspace' } });
    save();
    await waitFor(() => expect(setRetention).toHaveBeenLastCalledWith('workspace'));
  });

  it('leaves the period alone when it did not change, and refuses less than a day', async () => {
    renderModal({ initialTab: 'settings' });
    save();
    await waitFor(() => expect(updateChannel).toHaveBeenCalled());
    expect(setRetention).not.toHaveBeenCalled();

    fireEvent.change(screen.getByTestId('channel-retention-mode'), { target: { value: 'days' } });
    fireEvent.change(screen.getByTestId('channel-retention-days'), { target: { value: '0' } });
    expect(screen.getByText('At least one day')).toBeInTheDocument();
    expect(screen.getByTestId('channel-settings-save')).toBeDisabled();
  });
});

describe('ChannelDetailsModal privacy', () => {
  const typed = (channel_type: string, extra: Partial<Channel> = {}) =>
    ({ ...channel, channel_type, ...extra }) as unknown as Channel;

  it('gives the change to a workspace admin only, and never on a default or archived channel', () => {
    renderModal({ of: typed('public'), initialTab: 'settings' });
    expect(screen.queryByTestId('channel-settings-privacy')).not.toBeInTheDocument();
  });

  it.each([
    ['a default channel', typed('public', { is_default: true })],
    ['an archived channel', typed('private', { is_archived: true })],
  ])('shows no change on %s', (_, of) => {
    role = 'admin';
    renderModal({ of, initialTab: 'settings' });
    expect(screen.queryByTestId('channel-settings-privacy')).not.toBeInTheDocument();
  });

  it('makes a public channel private after one confirmation', async () => {
    role = 'owner';
    renderModal({ of: typed('public'), initialTab: 'settings' });
    expect(screen.getByTestId('channel-settings-privacy')).toHaveTextContent('Public');

    fireEvent.click(screen.getByTestId('channel-settings-privacy-change'));
    fireEvent.click(screen.getByTestId('channel-privacy-private-dialog-confirm'));

    await waitFor(() => expect(setPrivacy).toHaveBeenCalledWith(true));
    expect(updateChannel).not.toHaveBeenCalled();
  });

  it('says what a private channel holds and waits for the box before making it public', async () => {
    role = 'admin';
    renderModal({ of: typed('private'), initialTab: 'settings' });
    fireEvent.click(screen.getByTestId('channel-settings-privacy-change'));

    const dialog = screen.getByTestId('channel-privacy-public-dialog');
    expect(dialog).toHaveTextContent('12 messages and 1 file');
    const confirm = screen.getByTestId('channel-privacy-public-dialog-confirm');
    expect(confirm).toBeDisabled();

    fireEvent.click(screen.getByTestId('channel-privacy-public-understand'));
    fireEvent.click(confirm);
    await waitFor(() => expect(setPrivacy).toHaveBeenCalledWith(false));
  });

  it('keeps the button off while it is still counting', () => {
    role = 'admin';
    contents = undefined;
    renderModal({ of: typed('private'), initialTab: 'settings' });
    fireEvent.click(screen.getByTestId('channel-settings-privacy-change'));

    expect(screen.getByTestId('channel-privacy-public-dialog')).toHaveTextContent('Counting');
    fireEvent.click(screen.getByTestId('channel-privacy-public-understand'));
    expect(screen.getByTestId('channel-privacy-public-dialog-confirm')).toBeDisabled();
  });
});

describe('ChannelDetailsModal integrations', () => {
  it('lists what touches the channel, and only names it to a member', () => {
    integrations = { ...integrations, can_add_webhook: false };
    renderModal({ canModerate: false, initialTab: 'integrations' });

    const incoming = screen.getByTestId('channel-integration-incoming');
    expect(incoming).toHaveTextContent('CI');
    expect(incoming).toHaveTextContent('Added by Ana');
    expect(screen.getByTestId('channel-integration-outgoing')).toHaveTextContent('Mirror');
    expect(screen.getByTestId('channel-integration-command')).toHaveTextContent('/deploy');
    expect(screen.getByTestId('channel-integration-workflow')).toHaveTextContent('On · built by Ana');
    expect(screen.queryByTestId('channel-webhook-add')).not.toBeInTheDocument();
    expect(screen.queryByTestId('channel-webhook-rotate')).not.toBeInTheDocument();
    expect(screen.queryByTestId('channel-integration-workflow-open')).not.toBeInTheDocument();
  });

  it('adds an incoming webhook and shows its URL under it', async () => {
    integrations.incoming_webhooks.push({
      id: 'h-new',
      name: 'Alerts',
      description: null,
      created_by: 'u-ana',
      created_at: '2026-10-26T10:00:00Z',
    });
    renderModal({ initialTab: 'integrations' });
    fireEvent.change(screen.getByTestId('channel-webhook-name'), { target: { value: 'Alerts' } });
    fireEvent.click(screen.getByTestId('channel-webhook-add'));

    await waitFor(() =>
      expect(createHook).toHaveBeenCalledWith({
        hook_type: 'incoming_webhook',
        name: 'Alerts',
        config: { channel_id: 'ch-1' },
      }),
    );
    expect(revealHook).toHaveBeenCalledWith('h-new');
    await waitFor(() => expect(screen.getByTestId('channel-webhook-name')).toHaveValue(''));
    const urls = screen.getAllByTestId('channel-webhook-url-value');
    expect(urls).toHaveLength(1);
    expect(urls[0]).toHaveTextContent('https://chat.test/api/hooks/incoming/first');
  });

  it('makes a new URL for a webhook and shows it under that webhook', async () => {
    renderModal({ initialTab: 'integrations' });
    const incoming = screen.getByTestId('channel-integration-incoming');
    expect(within(incoming).queryByTestId('channel-webhook-url')).not.toBeInTheDocument();

    fireEvent.click(within(incoming).getByTestId('channel-webhook-rotate'));
    await waitFor(() =>
      expect(within(incoming).getByTestId('channel-webhook-url-value')).toHaveTextContent(
        'https://chat.test/api/hooks/incoming/second',
      ),
    );
    expect(rotateHook).toHaveBeenCalledWith('h-ci');
  });

  it('opens a workflow in the editor for someone who may build workflows', () => {
    const onOpenWorkflow = vi.fn();
    const onClose = renderModal({ initialTab: 'integrations', onOpenWorkflow });
    fireEvent.click(screen.getByTestId('channel-integration-workflow-open'));
    expect(onClose).toHaveBeenCalled();
    expect(onOpenWorkflow).toHaveBeenCalledWith('w-1');
  });

  it('links every entry to its page in the app directory', () => {
    const onOpenApp = vi.fn();
    const onClose = renderModal({ canModerate: false, initialTab: 'integrations', onOpenApp });
    const open = (qa: string) =>
      fireEvent.click(within(screen.getByTestId(qa)).getByTestId('channel-integration-details'));

    open('channel-integration-incoming');
    open('channel-integration-outgoing');
    open('channel-integration-command');
    open('channel-integration-workflow');
    expect(onClose).toHaveBeenCalledTimes(4);
    expect(onOpenApp.mock.calls.map(([app]) => app)).toEqual([
      { kind: 'incoming_webhook', id: 'h-ci' },
      { kind: 'outgoing_webhook', id: 'h-out' },
      { kind: 'slash_command', id: 'h-cmd' },
      { kind: 'workflow', id: 'w-1' },
    ]);
  });
});

describe('ChannelDetailsModal in Japanese', () => {
  const hostile = '<img src=x onerror="alert(1)"> {{name}}';

  beforeEach(async () => {
    await i18n.changeLanguage('ja');
  });
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('names the tabs and asks before archiving the channel by its name, as text', () => {
    renderModal({ of: { ...channel, name: hostile } });
    expect(tabs()).toEqual(['概要', '設定', 'アクティビティ', '連携']);
    fireEvent.click(screen.getByTestId('channel-about-archive'));
    expect(screen.getByRole('heading', { name: `#${hostile} をアーカイブしますか?` })).toBeInTheDocument();
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  it('counts the days messages are kept', () => {
    renderModal({ initialTab: 'settings' });
    expect(screen.getByRole('option', { name: 'ワークスペースの設定（365 日間）' })).toBeInTheDocument();
    expect(screen.getByRole('option', { name: '無期限に保存' })).toBeInTheDocument();
  });

  it('says how long the workspace keeps messages when channels may not choose', () => {
    retention = { overrides_allowed: false, workspace_message_days: 90, channel: null };
    renderModal({ initialTab: 'settings' });
    expect(screen.getByTestId('channel-retention-workspace').textContent).toBe(
      'このチャンネルのメッセージはワークスペースの設定に従い、90 日間保存されます。',
    );
  });

  it('says how many members there are', () => {
    renderModal({ initialTab: 'activity' });
    expect(screen.getAllByText('メンバー 6 人中')).toHaveLength(2);
  });

  it('names webhooks, workflows and who added them, as text', () => {
    memberName = hostile;
    integrations.incoming_webhooks[0].name = hostile;
    integrations.workflows[0].name = hostile;
    renderModal({ canModerate: false, initialTab: 'integrations', onOpenApp: vi.fn() });

    const incoming = screen.getByTestId('channel-integration-incoming');
    expect(
      within(incoming).getByText(`追加: ${hostile} さん ・ ${formatDateTime('2026-10-20T10:00:00Z')}`),
    ).toBeInTheDocument();
    expect(within(incoming).getByRole('button', { name: `${hostile} の詳細` })).toBeInTheDocument();
    expect(screen.getByText(`オン ・ 作成: ${hostile} さん`)).toBeInTheDocument();

    fireEvent.click(within(incoming).getByRole('button', { name: `${hostile} を削除` }));
    expect(screen.getByRole('heading', { name: `${hostile} を削除しますか?` })).toBeInTheDocument();
    expect(within(incoming).getByRole('button', { name: `${hostile} の URL を再発行` })).toBeInTheDocument();
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  it('asks before making a channel private, by its name as text', () => {
    role = 'admin';
    renderModal({ of: { ...channel, name: hostile, channel_type: 'public' }, initialTab: 'settings' });
    const field = screen.getByTestId('channel-settings-privacy');
    expect(field).toHaveTextContent(
      '公開範囲パブリック: ワークスペースの全員がこのチャンネルを見つけて、過去のメッセージを読めます。プライベートにする',
    );

    fireEvent.click(screen.getByTestId('channel-settings-privacy-change'));
    expect(
      screen.getByRole('heading', { name: `#${hostile} をプライベートにしますか?` }),
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        'これ以降は、参加しているメンバーだけが見られるようになります。それ以外の人は、すぐにこのチャンネルを見られなくなります。メッセージ、ファイル、検索結果、このチャンネルへのリンクも同様です。',
      ),
    ).toBeInTheDocument();
    expect(screen.getByTestId('channel-privacy-private-dialog-confirm')).toHaveTextContent(
      'プライベートにする',
    );
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  it('says what a private channel holds before making it public, by its name as text', () => {
    role = 'admin';
    renderModal({ of: { ...channel, name: hostile, channel_type: 'private' }, initialTab: 'settings' });
    expect(screen.getByTestId('channel-settings-privacy')).toHaveTextContent(
      '公開範囲プライベート: 参加しているメンバーだけが見られます。パブリックにする',
    );

    fireEvent.click(screen.getByTestId('channel-settings-privacy-change'));
    expect(screen.getByRole('heading', { name: `#${hostile} をパブリックにしますか?` })).toBeInTheDocument();
    expect(
      screen.getByText(
        'ワークスペースの全員がこのチャンネルを見つけて、これまでの履歴をすべて読めるようになります(12件のメッセージ、1件のファイル)。ゲストは引き続き、参加しているチャンネルだけを見られます。',
      ),
    ).toBeInTheDocument();
    expect(
      screen.getByText('プライベートで書かれた内容を、全員が読めるようになることを理解しました'),
    ).toBeInTheDocument();
    expect(screen.getByTestId('channel-privacy-public-dialog-confirm')).toHaveTextContent('パブリックにする');
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  it('says it is still counting', () => {
    role = 'admin';
    contents = undefined;
    renderModal({ of: { ...channel, channel_type: 'private' }, initialTab: 'settings' });
    fireEvent.click(screen.getByTestId('channel-settings-privacy-change'));
    expect(screen.getByText('件数を数えています…')).toBeInTheDocument();
  });
});
