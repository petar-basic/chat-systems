import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import i18n from '@/lib/i18n';
import type { ReactNode } from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { GeneralSection } from './GeneralSection';
import type { Workspace } from '@/stores/workspace';

const patch = vi.fn();
const upload = vi.fn();

vi.mock('react-router', () => ({ useNavigate: () => vi.fn() }));
vi.mock('@/features/workspace/hooks/useWorkspaces', () => ({
  useRestoreWorkspace: () => ({ mutateAsync: vi.fn(), isPending: false }),
}));
vi.mock('@/shared/hooks/useCurrentApi', () => ({
  getApiForInstance: () => ({
    typed: (call: (c: { PATCH: typeof patch }) => unknown) => call({ PATCH: patch }),
    upload: (...args: unknown[]) => upload(...args),
  }),
}));

const workspace = {
  id: 'ws-1',
  name: 'Dev Team',
  description: 'Where we build',
  instanceUrl: 'http://localhost:8080',
  settings: {},
} as unknown as Workspace;

function wrapper({ children }: { children: ReactNode }) {
  return <QueryClientProvider client={new QueryClient()}>{children}</QueryClientProvider>;
}

const bodyOf = (call: number) => patch.mock.calls[call][1].body;

describe('GeneralSection', () => {
  beforeEach(() => {
    patch.mockReset();
    patch.mockResolvedValue({ data: {} });
    upload.mockReset();
    upload.mockResolvedValue({ ...workspace, icon_url: '/api/files/download/icons/ws-1/logo.png' });
  });

  it('uploads an icon to the workspace, not as a file of it', async () => {
    render(<GeneralSection workspace={workspace} onDeleted={vi.fn()} canDelete />, { wrapper });
    const icon = new File([new Uint8Array([1])], 'logo.png', { type: 'image/png' });
    fireEvent.change(screen.getByLabelText('Workspace icon'), { target: { files: [icon] } });
    await waitFor(() => expect(upload).toHaveBeenCalledOnce());
    expect(upload).toHaveBeenCalledWith('/workspaces/ws-1/icon', expect.any(FormData));
    expect((upload.mock.calls[0][1] as FormData).get('file')).toBe(icon);
    expect(patch).not.toHaveBeenCalled();
  });

  it('saves a renamed workspace when the field is left, and only the name', async () => {
    render(<GeneralSection workspace={workspace} onDeleted={vi.fn()} canDelete />, { wrapper });
    const field = screen.getByLabelText(/Workspace name/);
    fireEvent.change(field, { target: { value: '  Platform  ' } });
    fireEvent.blur(field);
    await waitFor(() => expect(patch).toHaveBeenCalledOnce());
    expect(bodyOf(0)).toEqual({ name: 'Platform' });
    expect(await screen.findByText('Saved')).toBeInTheDocument();
  });

  it('saves nothing when a field is left unchanged, and puts back an emptied name', () => {
    render(<GeneralSection workspace={workspace} onDeleted={vi.fn()} canDelete />, { wrapper });
    fireEvent.blur(screen.getByLabelText(/Description/));
    const field = screen.getByLabelText(/Workspace name/);
    fireEvent.change(field, { target: { value: '   ' } });
    fireEvent.blur(field);
    expect(patch).not.toHaveBeenCalled();
    expect(field).toHaveValue('Dev Team');
  });

  it('saves a switch the moment it changes', async () => {
    render(<GeneralSection workspace={workspace} onDeleted={vi.fn()} canDelete />, { wrapper });
    fireEvent.change(screen.getByTestId('workspace-workflow-builders'), { target: { value: 'admins' } });
    await waitFor(() => expect(patch).toHaveBeenCalledOnce());
    expect(bodyOf(0)).toEqual({ workflow_builders: 'admins' });
  });

  it('saves who may broadcast and who may create each kind of channel, one rule at a time', async () => {
    render(
      <GeneralSection
        workspace={{ ...workspace, settings: { private_channel_creators: 'admins' } }}
        onDeleted={vi.fn()}
        canDelete
      />,
      { wrapper },
    );
    expect(screen.getByTestId('workspace-broadcast-mentions')).toHaveValue('members');
    expect(screen.getByTestId('workspace-private-channel-creators')).toHaveValue('admins');

    fireEvent.change(screen.getByTestId('workspace-broadcast-mentions'), { target: { value: 'admins' } });
    await waitFor(() => expect(patch).toHaveBeenCalledOnce());
    expect(bodyOf(0)).toEqual({ broadcast_mentions: 'admins' });

    fireEvent.change(screen.getByTestId('workspace-public-channel-creators'), {
      target: { value: 'admins' },
    });
    await waitFor(() => expect(patch).toHaveBeenCalledTimes(2));
    expect(bodyOf(1)).toEqual({ public_channel_creators: 'admins' });
    expect(screen.getByTestId('workspace-broadcast-mentions')).toHaveValue('admins');
  });

  it('keeps the danger zone from an admin who cannot delete the workspace', () => {
    render(<GeneralSection workspace={workspace} onDeleted={vi.fn()} canDelete={false} />, { wrapper });
    expect(screen.getByLabelText(/Workspace name/)).toHaveValue('Dev Team');
    expect(screen.queryByText('Danger zone')).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Delete workspace' })).not.toBeInTheDocument();
  });

  it('offers to restore a soft-deleted workspace only to someone who can delete it', () => {
    const deleted = { ...workspace, deleted_at: '2026-11-17T09:00:00Z' };
    const { rerender } = render(
      <GeneralSection workspace={deleted} onDeleted={vi.fn()} canDelete={false} />,
      {
        wrapper,
      },
    );
    expect(screen.queryByRole('button', { name: 'Restore workspace' })).not.toBeInTheDocument();

    rerender(<GeneralSection workspace={deleted} onDeleted={vi.fn()} canDelete />);
    expect(screen.getByRole('button', { name: 'Restore workspace' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Delete workspace' })).not.toBeInTheDocument();
  });
});

describe('GeneralSection in Japanese', () => {
  const hostile = '<img src=x onerror="alert(1)"> {{name}}';

  beforeEach(async () => {
    patch.mockReset();
    patch.mockResolvedValue({ data: {} });
    await i18n.changeLanguage('ja');
  });

  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('names every setting and who may use each permission', () => {
    render(<GeneralSection workspace={workspace} onDeleted={vi.fn()} canDelete />, { wrapper });

    expect(screen.getByLabelText(/ワークスペース名/)).toHaveValue('Dev Team');
    expect(screen.getByText('リンクのプレビュー')).toBeInTheDocument();
    expect(screen.getByText('GIF')).toBeInTheDocument();
    expect(screen.getByLabelText('GIF のレーティング')).toHaveDisplayValue('PG — 軽い表現を含む');
    expect(screen.getByRole('option', { name: 'R — 成人向け' })).toBeInTheDocument();
    expect(screen.getByText('権限')).toBeInTheDocument();
    expect(screen.getByLabelText('ワークフローを作成できる人')).toBeInTheDocument();
    expect(screen.getByLabelText('@channel、@here、@everyone を使える人')).toBeInTheDocument();
    expect(screen.getByLabelText('公開チャンネルを作成できる人')).toBeInTheDocument();
    expect(screen.getByLabelText('非公開チャンネルを作成できる人')).toBeInTheDocument();
    expect(screen.getByLabelText('メンバーを招待できる人')).toBeInTheDocument();
    expect(screen.getByLabelText('カスタムレスポンスを編集できる人')).toBeInTheDocument();
    expect(screen.getAllByRole('option', { name: 'メンバー全員' })).toHaveLength(6);
    expect(screen.getAllByRole('option', { name: '管理者のみ' })).toHaveLength(6);
    expect(screen.getByText('危険な操作')).toBeInTheDocument();
  });

  it('says when a change is saved', async () => {
    render(<GeneralSection workspace={workspace} onDeleted={vi.fn()} canDelete />, { wrapper });
    fireEvent.change(screen.getByTestId('workspace-inviters'), { target: { value: 'admins' } });
    expect(await screen.findByText('保存しました')).toBeInTheDocument();
  });

  it('says in Japanese when saving fails without a reason', async () => {
    patch.mockRejectedValue(new Error(''));
    render(<GeneralSection workspace={workspace} onDeleted={vi.fn()} canDelete />, { wrapper });
    fireEvent.change(screen.getByTestId('workspace-inviters'), { target: { value: 'admins' } });
    expect(await screen.findByText('設定を保存できませんでした')).toBeInTheDocument();
  });

  it('asks how to delete, naming the workspace as text', () => {
    render(<GeneralSection workspace={{ ...workspace, name: hostile }} onDeleted={vi.fn()} canDelete />, {
      wrapper,
    });
    fireEvent.click(screen.getByRole('button', { name: 'ワークスペースを削除' }));

    expect(screen.getByText(`「${hostile}」を削除`)).toBeInTheDocument();
    expect(screen.getByText('削除の方法を選んでください')).toBeInTheDocument();
    expect(screen.getAllByText('論理削除(非表示)').length).toBeGreaterThan(0);
    expect(screen.getByText('完全に削除')).toBeInTheDocument();
    expect(
      screen.getByText(
        'メンバーからは閲覧不能(非表示)になります。ワークスペースの管理者とサーバー管理者は引き続き閲覧可能です。オーナーとサーバー管理者は表示状態に戻すことができます。',
      ),
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'キャンセル' })).toBeInTheDocument();
    expect(document.querySelector('[onerror]')).toBeNull();
  });
});
