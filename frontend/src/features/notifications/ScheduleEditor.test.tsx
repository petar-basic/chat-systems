import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ScheduleEditor } from './ScheduleEditor';

const save = vi.fn();
let schedule: Record<string, unknown> = {};

vi.mock('@/features/notifications/hooks/useNotificationSchedule', () => ({
  useNotificationSchedule: () => ({ data: schedule }),
  useSetNotificationSchedule: () => ({ mutate: save }),
}));

const base = {
  enabled: true,
  days: [1, 2, 3, 4, 5],
  start_minute: 540,
  end_minute: 1080,
  timezone: 'Europe/Belgrade',
};
const saved = () => save.mock.calls[save.mock.calls.length - 1][0];

describe('ScheduleEditor', () => {
  beforeEach(() => {
    save.mockReset();
    schedule = { ...base };
  });

  it('shows only the switch until the schedule is on', () => {
    schedule = { ...base, enabled: false };
    render(<ScheduleEditor />);
    expect(screen.queryByRole('group', { name: 'Days' })).toBeNull();
    fireEvent.click(screen.getByTestId('schedule-enabled'));
    expect(saved()).toEqual({ ...base, enabled: true, timezone: undefined });
  });

  it('toggles days and offers the usual sets', () => {
    render(<ScheduleEditor />);
    expect(screen.getByRole('button', { name: 'Monday' })).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getByRole('button', { name: 'Weekdays' })).toHaveAttribute('aria-pressed', 'true');

    fireEvent.click(screen.getByRole('button', { name: 'Sunday' }));
    expect(saved().days).toEqual([1, 2, 3, 4, 5, 7]);
    fireEvent.click(screen.getByRole('button', { name: 'Wednesday' }));
    expect(saved().days).toEqual([1, 2, 4, 5]);
    fireEvent.click(screen.getByRole('button', { name: 'Every day' }));
    expect(saved().days).toEqual([1, 2, 3, 4, 5, 6, 7]);
  });

  it('keeps at least one day', () => {
    schedule = { ...base, days: [3] };
    render(<ScheduleEditor />);
    expect(screen.getByRole('button', { name: 'Wednesday' })).toBeDisabled();
  });

  it('never lets the window start and end at the same time', () => {
    render(<ScheduleEditor />);
    const end = screen.getByRole('combobox', { name: 'To' });
    expect(end.querySelector('option[value="540"]')).toBeNull();

    fireEvent.change(screen.getByRole('combobox', { name: 'From' }), { target: { value: '1080' } });
    expect(saved()).toMatchObject({ start_minute: 1080, end_minute: 1110 });
  });

  it('says which timezone the hours are in', () => {
    render(<ScheduleEditor />);
    expect(screen.getByTestId('schedule-timezone')).toHaveTextContent('Europe/Belgrade');
  });
});
