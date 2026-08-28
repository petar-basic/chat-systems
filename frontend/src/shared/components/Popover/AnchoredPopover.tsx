import { useEffect, useLayoutEffect, useRef, useState, type ReactNode, type RefObject } from 'react';
import { createPortal } from 'react-dom';
import { useEscapeToClose } from '@/shared/hooks/useEscapeToClose';

type Align = 'start' | 'end';

interface Props {
  anchorRef: RefObject<HTMLElement | null>;
  onClose: () => void;
  children: ReactNode;
  align?: Align;
  className?: string;
  dataQa?: string;
}

const MARGIN = 8;
const GAP = 4;

export function AnchoredPopover({
  anchorRef,
  onClose,
  children,
  align = 'end',
  className = '',
  dataQa,
}: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ top: number; left: number } | null>(null);
  useEscapeToClose(onClose);

  useLayoutEffect(() => {
    const anchor = anchorRef.current;
    const el = ref.current;
    if (!anchor || !el) return undefined;

    const compute = () => {
      const a = anchor.getBoundingClientRect();
      const w = el.offsetWidth;
      const h = el.offsetHeight;
      if (!w || !h) return;

      let top = a.bottom + GAP;
      if (top + h > window.innerHeight - MARGIN) {
        const above = a.top - h - GAP;
        top = above >= MARGIN ? above : window.innerHeight - h - MARGIN;
      }
      top = Math.max(MARGIN, top);

      let left = align === 'end' ? a.right - w : a.left;
      left = Math.max(MARGIN, Math.min(left, window.innerWidth - w - MARGIN));

      setPos({ top, left });
    };

    compute();
    const ro = new ResizeObserver(compute);
    ro.observe(el);
    window.addEventListener('resize', compute);
    window.addEventListener('scroll', compute, true);
    return () => {
      ro.disconnect();
      window.removeEventListener('resize', compute);
      window.removeEventListener('scroll', compute, true);
    };
  }, [anchorRef, align]);

  useEffect(() => {
    const onDown = (e: MouseEvent | TouchEvent) => {
      const target = e.target as Node;
      if (ref.current?.contains(target) || anchorRef.current?.contains(target)) return;
      onClose();
    };
    document.addEventListener('mousedown', onDown);
    document.addEventListener('touchstart', onDown);
    return () => {
      document.removeEventListener('mousedown', onDown);
      document.removeEventListener('touchstart', onDown);
    };
  }, [anchorRef, onClose]);

  return createPortal(
    <div
      ref={ref}
      data-qa={dataQa}
      style={{
        position: 'fixed',
        top: pos?.top ?? -9999,
        left: pos?.left ?? -9999,
        visibility: pos ? 'visible' : 'hidden',
      }}
      className={`z-70 ${className}`}
    >
      {children}
    </div>,
    document.body,
  );
}
