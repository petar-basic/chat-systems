import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
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
    fireEvent.click(screen.getByRole('button', { name: 'Resume notifications' }));
    expect(setDnd).toHaveBeenCalledWith(null);
  });

  it('says when a timer is the reason', () => {
    status = { dnd_until: inMinutes(30), schedule_paused_until: null };
    render(<PauseMenuItems onDone={vi.fn()} />);
    expect(screen.getByTestId('pause-status')).toHaveTextContent(/^Paused until /);
  });
});
