import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import i18n from '@/lib/i18n';
import { useHuddleStore } from '@/stores/huddle';
import { HuddleWindow } from './HuddleWindow';
import type { HuddleControls } from '../HuddleController';

vi.mock('@/features/workspace', () => ({ useWorkspaceMembers: () => ({ data: [] }) }));
vi.mock('@/features/huddle/hooks/useHuddle', () => ({ useInviteToHuddle: () => ({ mutate: vi.fn() }) }));
vi.mock('../hooks/useMediaDevices', () => ({
  useMediaDevices: () => ({ mics: [], cameras: [], speakers: [] }),
}));

const controls = new Proxy({}, { get: () => vi.fn() }) as unknown as HuddleControls;
const participant = (userId: string) => ({ userId }) as never;

describe('HuddleWindow in Japanese', () => {
  beforeEach(async () => {
    useHuddleStore.setState({
      active: {
        huddleId: 'h-1',
        workspaceId: 'ws-1',
        instanceUrl: 'http://localhost:3000',
        selfUserId: 'u-me',
        scope: { kind: 'channel', channelId: 'ch-1' },
      },
      participants: { 'u-a': participant('u-a'), 'u-b': participant('u-b') },
    });
    await i18n.changeLanguage('ja');
  });
  afterEach(async () => {
    useHuddleStore.setState({ active: null, participants: {} });
    await i18n.changeLanguage('en');
  });

  it('counts the people in the huddle and names the view buttons', () => {
    render(<HuddleWindow controls={controls} />);
    const window = screen.getByRole('dialog', { name: '通話' });
    expect(window.querySelector('span')?.textContent).toBe('通話 ・ 3人');
    expect(screen.getByRole('button', { name: '大きく表示' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '通話を全画面表示' })).toBeInTheDocument();
  });
});
