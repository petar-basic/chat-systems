import { useState } from 'react';
import { useLocation, useNavigationType } from 'react-router';

function historyIndex(): number {
  const state: unknown = window.history.state;
  if (state && typeof state === 'object' && 'idx' in state && typeof state.idx === 'number') return state.idx;
  return 0;
}

export function useHistoryReach(): { back: boolean; forward: boolean } {
  const { key } = useLocation();
  const type = useNavigationType();
  const index = historyIndex();
  const [furthest, setFurthest] = useState(index);
  const [seen, setSeen] = useState(key);
  let reach = furthest;
  if (seen !== key) {
    reach = type === 'PUSH' ? index : Math.max(furthest, index);
    setSeen(key);
    setFurthest(reach);
  }
  return { back: index > 0, forward: index < reach };
}
