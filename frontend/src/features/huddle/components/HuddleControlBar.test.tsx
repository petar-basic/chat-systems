import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import i18n from '@/lib/i18n';
import { useHuddleStore } from '@/stores/huddle';
import { HuddleControlBar } from './HuddleControlBar';
import type { HuddleControls } from '../HuddleController';

vi.mock('@/features/workspace', () => ({ useWorkspaceMembers: () => ({ data: [] }) }));
vi.mock('@/features/huddle/hooks/useHuddle', () => ({ useInviteToHuddle: () => ({ mutate: vi.fn() }) }));
vi.mock('../hooks/useMediaDevices', () => ({
  useMediaDevices: () => ({ mics: [], cameras: [], speakers: [] }),
}));

const controls = new Proxy({}, { get: () => vi.fn() }) as unknown as HuddleControls;
const active = {
  huddleId: 'h-1',
  workspaceId: 'ws-1',
  instanceUrl: 'http://localhost:3000',
  selfUserId: 'u-me',
  scope: { kind: 'channel' as const, channelId: 'ch-1' },
};

describe('HuddleControlBar in Japanese', () => {
  beforeEach(async () => {
    useHuddleStore.setState({
      localMuted: true,
      localCameraOn: false,
      localSharing: false,
      localHandRaised: false,
      background: 'none' as never,
      participants: {},
    });
    await i18n.changeLanguage('ja');
  });
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('names every control', () => {
    render(<HuddleControlBar controls={controls} active={active} />);
    for (const name of [
      'ミュート解除',
      'カメラをオン',
      '画面を共有',
      '手を挙げる',
      '背景をぼかす',
      'リアクションを送信',
      '通話に招待',
      'デバイス',
      '通話から退出',
    ]) {
      expect(screen.getByRole('button', { name })).toBeInTheDocument();
    }

    fireEvent.click(screen.getByRole('button', { name: '通話に招待' }));
    expect(screen.getByText('招待できるメンバーがいません')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'デバイス' }));
    expect(screen.getByLabelText('マイク')).toBeInTheDocument();
    expect(screen.getByLabelText('カメラ')).toBeInTheDocument();
    expect(screen.getAllByRole('option', { name: 'デバイスがありません' })).toHaveLength(2);
  });
});
