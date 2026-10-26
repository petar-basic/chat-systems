import { useEffect, useRef } from 'react';

type Handler = { current: () => void };

const open: Handler[] = [];

function onKey(e: KeyboardEvent) {
  if (e.key === 'Escape') open[open.length - 1]?.current();
}

export function useEscapeToClose(onClose: () => void, active = true) {
  const latest = useRef(onClose);

  useEffect(() => {
    latest.current = onClose;
  });

  useEffect(() => {
    if (!active) return undefined;
    const handler = latest;
    if (open.length === 0) document.addEventListener('keydown', onKey);
    open.push(handler);
    return () => {
      open.splice(open.lastIndexOf(handler), 1);
      if (open.length === 0) document.removeEventListener('keydown', onKey);
    };
  }, [active]);
}
