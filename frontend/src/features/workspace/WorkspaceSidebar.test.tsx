import { describe, it, expect, vi, afterEach, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import i18n from '@/lib/i18n';
import { useInstanceStore } from '@/stores/instances';
import { useWsStatusStore } from '@/stores/wsStatus';
import type { Workspace } from '@/stores/workspace';
import WorkspaceSidebar from './WorkspaceSidebar';

vi.mock('@/features/notifications', () => ({ useWorkspaceUnreadCounts: () => ({ 'ws-2': 3 }) }));
vi.mock('./hooks/useDesktopRail', () => ({ useDesktopRail: () => undefined }));
vi.mock('@/lib/instances', () => ({ instanceManager: { get: vi.fn() } }));

const url = 'https://chat.example.com';
const hostile = '<img src=x onerror="alert(1)"> {{name}}';
const workspace = (id: string, name: string) =>
  ({ id, name, icon_url: null, instanceUrl: url }) as unknown as Workspace;

function renderRail() {
  render(
    <WorkspaceSidebar
      workspaces={[workspace('ws-1', hostile), workspace('ws-2', hostile)]}
      deletedWorkspaces={[workspace('ws-3', hostile)]}
      currentWorkspaceId="ws-1"
      onSelectWorkspace={vi.fn()}
      onCreateWorkspace={vi.fn()}
      onLeaveWorkspace={vi.fn()}
      onAddInstance={vi.fn()}
      onImportFromSlack={vi.fn()}
    />,
  );
}

describe('WorkspaceSidebar', () => {
  beforeEach(() => {
    useInstanceStore.setState({ instances: [{ url } as never] });
    useWsStatusStore.setState({ statuses: { [url]: 'connected' } });
  });

  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('in Japanese names the workspaces, the server and what it offers, with names as text', async () => {
    await i18n.changeLanguage('ja');
    renderRail();

    expect(screen.getByRole('navigation', { name: 'ワークスペース' })).toBeInTheDocument();
    expect(screen.getByRole('img', { name: 'chat.example.com: 接続中' })).toBeInTheDocument();
    const [current, other] = screen.getAllByTestId('workspace-tile');
    expect(current).toHaveAttribute('data-tooltip', `${hostile} · chat.example.com`);
    expect(other).toHaveAttribute('data-tooltip', `${hostile} · chat.example.com · 未読 3件`);
    expect(screen.getByLabelText('未読の通知 3件')).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: `${hostile}(削除済み) · chat.example.com` }),
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'サーバーを追加' })).toBeInTheDocument();
    expect(document.querySelector('img')).toBeNull();

    fireEvent.click(screen.getByRole('button', { name: 'chat.example.com にワークスペースを作成' }));
    expect(screen.getByRole('heading', { name: 'ワークスペースを作成' })).toBeInTheDocument();
    expect(screen.getByText('代わりに Slack から取り込む')).toBeInTheDocument();
    expect(screen.getByLabelText(/ワークスペース名/)).toHaveAttribute('placeholder', '例: 営業部');
  });
});
