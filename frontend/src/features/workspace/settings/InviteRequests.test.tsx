import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import i18n from '@/lib/i18n';
import { formatDay } from '@/lib/datetime';
import type { InviteRequest } from '@/features/workspace/hooks/useInviteRequests';
import { InviteRequests } from './InviteRequests';
import { MyInviteRequests } from './MyInviteRequests';

const hostile = '<img src=x onerror="alert(1)"> {{name}}';

const requests = vi.hoisted(() => ({ list: [] as InviteRequest[] }));

vi.mock('@/features/workspace/hooks/useInviteRequests', () => ({
  useInviteRequests: () => ({ data: requests.list }),
  useApproveInviteRequest: () => ({ mutate: vi.fn(), isPending: false }),
  useDeclineInviteRequest: () => ({ mutateAsync: vi.fn() }),
}));

const request = (over: Partial<InviteRequest> = {}): InviteRequest => ({
  id: 'r1',
  workspace_id: 'w1',
  requested_by: 'u1',
  requester_name: 'Ana',
  emails: ['bo@example.com'],
  role: 'member',
  channel_ids: [],
  channel_names: [],
  status: 'pending',
  created_at: '2026-11-10T09:00:00Z',
  expires_at: '2026-12-10T09:00:00Z',
  ...over,
});

describe('invite requests in Japanese', () => {
  beforeEach(async () => {
    await i18n.changeLanguage('ja');
  });

  afterEach(async () => {
    requests.list = [];
    await i18n.changeLanguage('en');
  });

  it('shows what an admin decides on, with the requester and reason as text', () => {
    requests.list = [
      request({
        requester_name: hostile,
        reason: `${hostile} {{reason}}`,
        role: 'guest',
        guest_kind: 'single',
        guest_until: '2026-12-01T00:00:00Z',
        channel_names: ['design'],
      }),
      request({ id: 'r2' }),
    ];
    render(<InviteRequests workspaceId="w1" />);

    expect(screen.getByText('招待のリクエスト: 2件')).toBeInTheDocument();
    expect(
      screen.getByText(`1チャンネルのみのゲスト · ${formatDay('2026-12-01T00:00:00Z')} まで · #design`),
    ).toBeInTheDocument();
    expect(screen.getByText('メンバーとして')).toBeInTheDocument();
    expect(
      screen.getByText(`${hostile} さんのリクエスト · ${formatDay('2026-12-10T09:00:00Z')} に期限切れ`),
    ).toBeInTheDocument();
    expect(screen.getByText(`「${hostile} {{reason}}」`)).toBeInTheDocument();
    expect(screen.getAllByRole('button', { name: '承認' })).toHaveLength(2);
    expect(screen.getAllByRole('button', { name: '編集' })).toHaveLength(2);
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  it('asks before declining, naming the requester as text', () => {
    requests.list = [request({ requester_name: hostile })];
    render(<InviteRequests workspaceId="w1" />);

    fireEvent.click(screen.getByRole('button', { name: '却下' }));

    expect(screen.getByText(`${hostile} さんのリクエストを却下しますか?`)).toBeInTheDocument();
    expect(
      screen.getByText(
        '却下したことは、アクティビティで相手に伝わります。メモを書いた場合は、それも伝わります。',
      ),
    ).toBeInTheDocument();
    expect(screen.getByText(`${hostile} さんへのメモ`)).toBeInTheDocument();
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  it('lists the requests a member is waiting on', () => {
    requests.list = [request(), request({ id: 'r2', role: 'guest' })];
    render(<MyInviteRequests workspaceId="w1" />);

    expect(screen.getByText('管理者の対応待ちのリクエスト: 2件')).toBeInTheDocument();
    const sent = `${formatDay('2026-11-10T09:00:00Z')} に送信`;
    expect(screen.getByText(`メンバーとして · ${sent}`)).toBeInTheDocument();
    expect(screen.getByText(`ゲストとして · ${sent}`)).toBeInTheDocument();
  });
});
