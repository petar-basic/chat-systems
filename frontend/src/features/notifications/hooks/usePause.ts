import { useEffect, useState } from 'react';
import { currentPause, type Pause } from '@/lib/pause';
import { useDndStatus } from './useNotifications';

const LONGEST_TIMER_MS = 2 ** 31 - 1;

export function usePause(): Pause | null {
  const { data: status } = useDndStatus();
  const [now, setNow] = useState(() => Date.now());
  const pause = currentPause(status, now);
  const until = pause?.until;

  useEffect(() => {
    if (!until) return;
    const wait = Math.min(Math.max(Date.parse(until) - Date.now(), 0) + 1000, LONGEST_TIMER_MS);
    const timer = setTimeout(() => setNow(Date.now()), wait);
    return () => clearTimeout(timer);
  }, [until]);

  return pause;
}
