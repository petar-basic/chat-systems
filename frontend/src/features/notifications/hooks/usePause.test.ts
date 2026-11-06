import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';
import { usePause } from './usePause';

let status: { dnd_until: string | null; schedule_paused_until: string | null } | undefined;

vi.mock('./useNotifications', () => ({ useDndStatus: () => ({ data: status }) }));

describe('usePause', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2026-11-06T12:00:00Z'));
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('is nothing while notifications run', () => {
    status = { dnd_until: null, schedule_paused_until: null };
    expect(renderHook(() => usePause()).result.current).toBeNull();
  });

  it('ends on its own when the pause runs out', () => {
    status = { dnd_until: '2026-11-06T12:30:00Z', schedule_paused_until: null };
    const { result } = renderHook(() => usePause());
    expect(result.current).toEqual({ until: '2026-11-06T12:30:00Z', bySchedule: false });

    act(() => {
      vi.advanceTimersByTime(29 * 60000);
    });
    expect(result.current).not.toBeNull();

    act(() => {
      vi.advanceTimersByTime(2 * 60000);
    });
    expect(result.current).toBeNull();
  });
});
