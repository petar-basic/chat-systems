import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import i18n from '@/lib/i18n';
import WorkspacePage from './WorkspacePage';

const hostile = '<img src=x onerror="alert(1)"> {{name}}';
const { none } = vi.hoisted(() => ({ none: () => null }));

const restore = { mutate: vi.fn(), isPending: false };
let role = 'member';
let canPost = true;
let controller: Record<string, unknown> = {};

vi.mock('@/features/workspace', async (original) => ({
  ...(await original<typeof import('@/features/workspace')>()),
  DesktopTopBar: none,
  WorkspaceSidebar: none,
  WorkspaceRightPanels: none,
  WorkspaceSettingsModal: none,
  LeaveWorkspaceDialog: none,
  WorkspaceChecklist: none,
  useCurrentWorkspaceRole: () => ({ role }),
  useSettingsDialog: () => ({ open: vi.fn(), openApp: vi.fn(), close: vi.fn(), dialog: null }),
  useWorkspaceController: () => controller,
  useWorkspaceCreation: () => ({}),
}));

vi.mock('@/features/channel', async (original) => ({
  ...(await original<typeof import('@/features/channel')>()),
  ChannelSidebar: none,
  ChannelHeader: none,
  ChannelBookmarksBar: none,
  ChannelIntro: none,
  ChannelPreviewBar: none,
  ChannelArchivedBar: none,
  CreateChannelModal: none,
}));

vi.mock('@/features/channel/hooks/useChannelModeration', () => ({
  useChannelModeration: () => ({
    canPost,
    canModerate: false,
    canBroadcast: false,
    members: [],
    myRole: 'member',
    workspaceRole: role,
  }),
}));

vi.mock('@/features/messaging', async (original) => ({
  ...(await original<typeof import('@/features/messaging')>()),
  CommandAnswer: none,
  MessageList: none,
  MessageInput: none,
}));

vi.mock('@/features/messaging/TypingIndicator', () => ({ default: none }));
vi.mock('@/features/messaging/blocks/views/AppViewHost', () => ({ AppViewHost: none }));
vi.mock('@/features/instance/InstallAppBanner', () => ({ default: none }));
vi.mock('@/shared/components/ConnectionBanner/ConnectionBanner', () => ({ ConnectionBanner: none }));

vi.mock('@/features/profile', async (original) => ({
  ...(await original<typeof import('@/features/profile')>()),
  TimezonePrompt: none,
  usePreferencesSync: () => {},
  useWorkspaceTheme: () => null,
}));

vi.mock('@/features/huddle', async (original) => ({
  ...(await original<typeof import('@/features/huddle')>()),
  useActiveHuddlesSync: () => {},
}));

vi.mock('@/features/drafts', async (original) => ({
  ...(await original<typeof import('@/features/drafts')>()),
  useDraftSync: () => {},
}));

const workspace = { id: 'w1', name: 'Northwind', instanceUrl: 'https://chat.example.com', settings: {} };
const general = { id: 'c1', name: 'general', workspace_id: 'w1', channel_type: 'public', is_archived: false };

function setUp(overrides: Record<string, unknown> = {}) {
  controller = {
    panel: { active: null, open: vi.fn(), toggle: vi.fn(), close: vi.fn(), openThread: vi.fn() },
    currentWorkspace: workspace,
    currentChannel: general,
    user: { id: 'me', instance_role: 'member' },
    conversations: [],
    channels: [general],
    workspaceMembers: [],
    workspaces: [workspace],
    deletedWorkspaces: [],
    attachments: { add: vi.fn() },
    restoreWorkspace: restore,
    setMobileNavOpen: vi.fn(),
    mobileNavOpen: false,
    noWorkspaces: false,
    channelsPending: false,
    previewing: false,
    ...overrides,
  };
}

const page = () => render(<WorkspacePage />);

describe('WorkspacePage', () => {
  beforeEach(() => {
    restore.mutate.mockReset();
    restore.isPending = false;
    role = 'member';
    canPost = true;
    setUp();
  });

  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('names the conversation area', () => {
    page();
    expect(screen.getByRole('main', { name: 'Conversation' })).toBeInTheDocument();
  });

  it('lets an owner restore a soft-deleted workspace', () => {
    role = 'owner';
    setUp({ currentWorkspace: { ...workspace, deleted_at: '2026-11-01T00:00:00Z' } });
    page();
    expect(
      screen.getByText('This workspace has been soft-deleted and is not visible to regular members.'),
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Restore workspace' }));
    expect(restore.mutate).toHaveBeenCalledWith({
      workspaceId: 'w1',
      instanceUrl: 'https://chat.example.com',
    });
  });

  it('says so while restoring', () => {
    role = 'owner';
    restore.isPending = true;
    setUp({ currentWorkspace: { ...workspace, deleted_at: '2026-11-01T00:00:00Z' } });
    page();
    expect(screen.getByRole('button', { name: 'Restoring…' })).toBeDisabled();
  });

  it('tells an admin without the restore button', () => {
    role = 'admin';
    setUp({ currentWorkspace: { ...workspace, deleted_at: '2026-11-01T00:00:00Z' } });
    page();
    expect(
      screen.getByText('This workspace has been soft-deleted and is not visible to regular members.'),
    ).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Restore workspace' })).toBeNull();
  });

  it('says who can post where you cannot', () => {
    canPost = false;
    page();
    expect(screen.getByTestId('channel-read-only')).toHaveTextContent(
      'Only admins can post in this channel.',
    );
  });

  it('says when there are no channels', () => {
    setUp({ currentChannel: null, channels: [] });
    page();
    expect(screen.getByText('No channels yet')).toBeInTheDocument();
    expect(screen.getByText('Create one to start the conversation.')).toBeInTheDocument();
  });

  describe('in Japanese', () => {
    beforeEach(async () => {
      await i18n.changeLanguage('ja');
    });

    it('names the conversation area', () => {
      page();
      expect(screen.getByRole('main', { name: '会話' })).toBeInTheDocument();
    });

    it('lets an owner restore a soft-deleted workspace', () => {
      role = 'owner';
      setUp({ currentWorkspace: { ...workspace, deleted_at: '2026-11-01T00:00:00Z' } });
      page();
      expect(
        screen.getByText(
          'このワークスペースは論理削除(非表示)されています。一般のメンバーからは見えません。',
        ),
      ).toBeInTheDocument();
      fireEvent.click(screen.getByRole('button', { name: 'ワークスペースを表示状態に戻す' }));
      expect(restore.mutate).toHaveBeenCalled();
    });

    it('says so while restoring', () => {
      role = 'owner';
      restore.isPending = true;
      setUp({ currentWorkspace: { ...workspace, deleted_at: '2026-11-01T00:00:00Z' } });
      page();
      expect(screen.getByRole('button', { name: '表示状態に戻しています…' })).toBeDisabled();
    });

    it('says who can post where you cannot', () => {
      canPost = false;
      page();
      expect(screen.getByTestId('channel-read-only')).toHaveTextContent(
        'このチャンネルには、ワークスペースとこのチャンネルの管理者だけが投稿できます。',
      );
    });

    it('says when there are no channels', () => {
      setUp({ currentChannel: null, channels: [] });
      page();
      expect(screen.getByText('チャンネルはまだありません')).toBeInTheDocument();
      expect(screen.getByText('チャンネルを作成して、会話を始めましょう。')).toBeInTheDocument();
    });

    it('has Japanese for every one', () => {
      const leaves = (o: object, at = ''): string[] =>
        Object.entries(o).flatMap(([k, v]) =>
          typeof v === 'object' && v ? leaves(v as object, `${at}${k}.`) : [`${at}${k}`],
        );
      const english = leaves(i18n.getResource('en', 'workspace', 'page'));
      const japanese = leaves(i18n.getResource('ja', 'workspace', 'page'));
      expect(english.length).toBeGreaterThan(0);
      expect(japanese).toEqual(english);
    });
  });

  describe('with strings translated as markup', () => {
    const keys = [
      'page.deleted.notice',
      'page.deleted.restore',
      'page.readOnly',
      'page.noChannels.title',
      'page.noChannels.hint',
    ] as const;
    let saved: string[] = [];

    beforeEach(() => {
      saved = keys.map((k) => i18n.t(k, { ns: 'workspace' }));
      keys.forEach((k) => i18n.addResource('en', 'workspace', k, hostile));
    });

    afterEach(() => {
      keys.forEach((k, i) => i18n.addResource('en', 'workspace', k, saved[i]));
    });

    it('shows the deleted notice and its button as text', () => {
      role = 'owner';
      setUp({ currentWorkspace: { ...workspace, deleted_at: '2026-11-01T00:00:00Z' } });
      page();
      expect(screen.getByText(hostile, { selector: 'p' })).toBeInTheDocument();
      expect(screen.getByRole('button', { name: hostile })).toBeInTheDocument();
      expect(document.querySelector('[onerror]')).toBeNull();
    });

    it('shows the read-only notice as text', () => {
      canPost = false;
      page();
      expect(screen.getByTestId('channel-read-only').textContent).toBe(hostile);
      expect(document.querySelector('[onerror]')).toBeNull();
    });

    it('shows the no-channels text as text', () => {
      setUp({ currentChannel: null, channels: [] });
      page();
      expect(screen.getAllByText(hostile)).toHaveLength(2);
      expect(document.querySelector('[onerror]')).toBeNull();
    });
  });
});
