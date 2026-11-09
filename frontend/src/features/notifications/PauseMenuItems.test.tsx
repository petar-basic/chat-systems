import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import i18n from '@/lib/i18n';
import { useWorkspaceStore } from '@/stores/workspace';
import { PauseMenuItems } from './PauseMenuItems';

const setDnd = vi.fn();
let status: Record<string, string | null> | undefined;

vi.mock('@/features/notifications/hooks/useNotifications', () => ({
  useDndStatus: () => ({ data: status }),
  useSetDnd: () => ({ mutate: setDnd }),
}));

const inMinutes = (minutes: number) => new Date(Date.now() + minutes * 60000).toISOString();

describe('PauseMenuItems', () => {
  beforeEach(() => {
    setDnd.mockReset();
    status = { dnd_until: null, schedule_paused_until: null };
    useWorkspaceStore.setState({ currentWorkspace: { id: 'ws-1', name: 'Acme' } as never });
  });

  it('names the workspace it pauses', () => {
    render(<PauseMenuItems onDone={vi.fn()} />);
    expect(screen.getByTestId('pause-heading')).toHaveTextContent('Pause notifications in Acme');
  });

  it('offers pauses when notifications are on', () => {
    const done = vi.fn();
    render(<PauseMenuItems onDone={done} />);
    fireEvent.click(screen.getByRole('button', { name: 'For 1 hour' }));
    expect(setDnd).toHaveBeenCalledWith(expect.any(String));
    expect(done).toHaveBeenCalled();
  });

  it('says when the schedule is the reason, and resumes', () => {
    status = { dnd_until: null, schedule_paused_until: inMinutes(90) };
    render(<PauseMenuItems onDone={vi.fn()} />);
    expect(screen.getByTestId('pause-status')).toHaveTextContent(/^Paused by your schedule until /);
    fireEvent.click(screen.getByRole('button', { name: 'Resume notifications in Acme' }));
    expect(setDnd).toHaveBeenCalledWith(null);
  });

  it('says when a timer is the reason', () => {
    status = { dnd_until: inMinutes(30), schedule_paused_until: null };
    render(<PauseMenuItems onDone={vi.fn()} />);
    expect(screen.getByTestId('pause-status')).toHaveTextContent(/^Paused until /);
  });
});

describe('PauseMenuItems in Japanese', () => {
  const workspace = '<img src=x onerror="alert(1)"> {{name}}';

  beforeEach(async () => {
    setDnd.mockReset();
    status = { dnd_until: null, schedule_paused_until: null };
    useWorkspaceStore.setState({ currentWorkspace: { id: 'ws-1', name: workspace } as never });
    await i18n.changeLanguage('ja');
  });

  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('names the workspace as text and offers the pauses', () => {
    render(<PauseMenuItems onDone={vi.fn()} />);
    expect(screen.getByTestId('pause-heading')).toHaveTextContent(`${workspace} の通知を一時停止`);
    expect(document.querySelector('img')).toBeNull();
    for (const name of ['30分', '1時間', '2時間', '明日の朝まで']) {
      expect(screen.getByRole('button', { name })).toBeInTheDocument();
    }
  });

  it('says the schedule paused it, and resumes the workspace by name', () => {
    status = { dnd_until: null, schedule_paused_until: inMinutes(90) };
    render(<PauseMenuItems onDone={vi.fn()} />);
    expect(screen.getByTestId('pause-status')).toHaveTextContent(/^通知設定により .+ まで一時停止中$/);
    fireEvent.click(screen.getByRole('button', { name: `${workspace} の通知を再開` }));
    expect(setDnd).toHaveBeenCalledWith(null);
    expect(document.querySelector('img')).toBeNull();
  });

  it('leaves the workspace out when there is none', () => {
    useWorkspaceStore.setState({ currentWorkspace: null });
    render(<PauseMenuItems onDone={vi.fn()} />);
    expect(screen.getByTestId('pause-heading')).toHaveTextContent(/^通知を一時停止$/);
  });
});
