import { useEffect, useRef, type ReactNode, type RefObject } from 'react';
import { createPortal } from 'react-dom';
import { autoUpdate, flip, offset, shift, useFloating } from '@floating-ui/react-dom';
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
  useEscapeToClose(onClose);

  const { refs, floatingStyles, isPositioned } = useFloating({
    placement: align === 'end' ? 'bottom-end' : 'bottom-start',
    strategy: 'fixed',
    middleware: [offset(GAP), flip({ padding: MARGIN }), shift({ padding: MARGIN })],
    whileElementsMounted: autoUpdate,
  });

  useEffect(() => {
    refs.setReference(anchorRef.current);
  }, [anchorRef, refs]);

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
      ref={(node) => {
        ref.current = node;
        refs.setFloating(node);
      }}
      data-qa={dataQa}
      style={{ ...floatingStyles, visibility: isPositioned ? 'visible' : 'hidden' }}
      className={`z-70 ${className}`}
    >
      {children}
    </div>,
    document.body,
  );
}
