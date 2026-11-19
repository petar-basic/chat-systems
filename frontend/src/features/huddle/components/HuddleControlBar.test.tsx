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

describe('HuddleControlBar drawing switch', () => {
  beforeEach(() => {
    useHuddleStore.setState({
      localMuted: false,
      localSharing: false,
      localDrawing: true,
      participants: {},
    });
  });

  it('shows only while sharing and says what a press will do', () => {
    const toggleDrawing = vi.fn();
    const withDrawing = { ...controls, toggleDrawing } as HuddleControls;
    const { rerender } = render(<HuddleControlBar controls={withDrawing} active={active} />);
    expect(screen.queryByTestId('huddle-drawing-toggle')).toBeNull();

    useHuddleStore.setState({ localSharing: true });
    rerender(<HuddleControlBar controls={withDrawing} active={active} />);
    fireEvent.click(screen.getByRole('button', { name: 'Stop others drawing on your screen' }));
    expect(toggleDrawing).toHaveBeenCalledOnce();

    useHuddleStore.setState({ localDrawing: false });
    rerender(<HuddleControlBar controls={withDrawing} active={active} />);
    expect(screen.getByRole('button', { name: 'Let others draw on your screen' })).toHaveAttribute(
      'aria-pressed',
      'false',
    );
  });
});

describe('HuddleControlBar drawing and thread in Japanese', () => {
  beforeEach(async () => {
    useHuddleStore.setState({ localMuted: false, localSharing: true, localDrawing: true, participants: {} });
    await i18n.changeLanguage('ja');
  });
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('says what the drawing switch will do', () => {
    const { rerender } = render(<HuddleControlBar controls={controls} active={active} />);
    expect(screen.getByRole('button', { name: 'ほかの人の描き込みを止める' })).toBeInTheDocument();

    useHuddleStore.setState({ localDrawing: false });
    rerender(<HuddleControlBar controls={controls} active={active} />);
    expect(screen.getByRole('button', { name: 'ほかの人に画面への描き込みを許可' })).toBeInTheDocument();
  });

  it('opens and closes the thread, counting unread replies', () => {
    const thread = { open: false, unread: 2, onToggle: vi.fn() };
    const { rerender } = render(<HuddleControlBar controls={controls} active={active} thread={thread} />);
    const toggle = screen.getByRole('button', { name: '通話のスレッドを開く、未読の返信 2 件' });
    expect(toggle).toHaveAttribute('data-tooltip', '通話のスレッドを開く');

    rerender(<HuddleControlBar controls={controls} active={active} thread={{ ...thread, open: true }} />);
    expect(screen.getByRole('button', { name: '通話のスレッドを閉じる' })).toBeInTheDocument();
  });
});

describe('the huddle strings', () => {
  function leaves(value: unknown, prefix = ''): string[] {
    if (typeof value !== 'object' || value === null) return [prefix];
    return Object.entries(value).flatMap(([key, child]) => leaves(child, prefix ? `${prefix}.${key}` : key));
  }
  const counted = (keys: string[]) =>
    [...new Set(keys.map((key) => key.replace(/_(one|other)$/, '')))].sort();

  it('has Japanese for every one', () => {
    const english = leaves(i18n.getResource('en', 'huddle', ''));
    const japanese = leaves(i18n.getResource('ja', 'huddle', ''));
    expect(english.length).toBeGreaterThan(0);
    expect(counted(japanese)).toEqual(counted(english));
  });
});
