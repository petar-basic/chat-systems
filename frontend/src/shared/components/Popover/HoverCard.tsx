import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { autoUpdate, flip, offset, shift, useFloating, type Placement } from '@floating-ui/react-dom';

interface Props {
  content: ReactNode;
  children: ReactNode;
  placement?: Placement;
  openDelay?: number;
  closeDelay?: number;
  panelClassName?: string;
  triggerClassName?: string;
  dataQa?: string;
}

const MARGIN = 8;
const GAP = 6;

function isCoarsePointer(): boolean {
  return typeof window !== 'undefined' && !!window.matchMedia?.('(pointer: coarse)').matches;
}

export function HoverCard({
  content,
  children,
  placement = 'top',
  openDelay = 350,
  closeDelay = 180,
  panelClassName = '',
  triggerClassName = '',
  dataQa,
}: Props) {
  const [open, setOpen] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const triggerRef = useRef<HTMLSpanElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);

  const { refs, floatingStyles, isPositioned } = useFloating({
    placement,
    strategy: 'fixed',
    middleware: [offset(GAP), flip({ padding: MARGIN }), shift({ padding: MARGIN })],
    whileElementsMounted: autoUpdate,
  });

  useEffect(() => {
    refs.setReference(triggerRef.current);
  }, [refs]);

  const clearTimer = useCallback(() => {
    if (timer.current) clearTimeout(timer.current);
    timer.current = null;
  }, []);

  const schedule = useCallback(
    (next: boolean, delay: number) => {
      clearTimer();
      timer.current = setTimeout(() => setOpen(next), delay);
    },
    [clearTimer],
  );

  const closeNow = useCallback(() => {
    clearTimer();
    setOpen(false);
  }, [clearTimer]);

  useEffect(() => clearTimer, [clearTimer]);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') closeNow();
    };
    const onScroll = (e: Event) => {
      if (panelRef.current?.contains(e.target as Node)) return;
      closeNow();
    };
    const onPointerDown = (e: PointerEvent) => {
      const target = e.target as Node;
      if (panelRef.current?.contains(target) || triggerRef.current?.contains(target)) return;
      closeNow();
    };
    document.addEventListener('keydown', onKey);
    document.addEventListener('scroll', onScroll, true);
    document.addEventListener('pointerdown', onPointerDown);
    return () => {
      document.removeEventListener('keydown', onKey);
      document.removeEventListener('scroll', onScroll, true);
      document.removeEventListener('pointerdown', onPointerDown);
    };
  }, [open, closeNow]);

  return (
    <>
      <span
        ref={triggerRef}
        className={triggerClassName}
        onPointerEnter={(e) => {
          if (e.pointerType === 'mouse') schedule(true, openDelay);
        }}
        onPointerLeave={(e) => {
          if (e.pointerType === 'mouse') schedule(false, closeDelay);
        }}
        onClick={() => {
          if (!isCoarsePointer()) return;
          clearTimer();
          setOpen((v) => !v);
        }}
        onFocus={(e) => {
          if (e.target.matches(':focus-visible')) {
            clearTimer();
            setOpen(true);
          }
        }}
        onBlur={(e) => {
          if (!e.currentTarget.contains(e.relatedTarget as Node | null)) schedule(false, closeDelay);
        }}
      >
        {children}
      </span>

      {open &&
        createPortal(
          <div
            ref={(node) => {
              panelRef.current = node;
              refs.setFloating(node);
            }}
            role="tooltip"
            data-qa={dataQa}
            style={{ ...floatingStyles, visibility: isPositioned ? 'visible' : 'hidden' }}
            className={`z-70 ${panelClassName}`}
            onPointerEnter={clearTimer}
            onPointerLeave={(e) => {
              if (e.pointerType === 'mouse') schedule(false, closeDelay);
            }}
          >
            {content}
          </div>,
          document.body,
        )}
    </>
  );
}
