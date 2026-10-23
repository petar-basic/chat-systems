import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { useEscapeToClose } from '@/shared/hooks/useEscapeToClose';
import { DismissContext } from './dismiss';

interface OverlayProps {
  label: string;
  onClose: () => void;
  children: ReactNode;
  className: string;
  backdropClassName?: string;
  dataQa?: string;
}

const EXIT_MS = 100;

const FOCUSABLE =
  'a[href],button:not([disabled]),textarea:not([disabled]),input:not([disabled]),select:not([disabled]),[tabindex]:not([tabindex="-1"])';

export function Overlay({ label, onClose, children, className, backdropClassName, dataQa }: OverlayProps) {
  const ref = useRef<HTMLDivElement>(null);
  const [closing, setClosing] = useState(false);
  const closed = useRef(false);

  const finish = useCallback(() => {
    if (closed.current) return;
    closed.current = true;
    onClose();
  }, [onClose]);

  const dismiss = useCallback(() => {
    const node = ref.current;
    const animation = node ? getComputedStyle(node).animationName : '';
    if (!animation || animation === 'none') {
      onClose();
      return;
    }
    setClosing(true);
  }, [onClose]);

  useEffect(() => {
    if (!closing) return undefined;
    const fallback = window.setTimeout(finish, EXIT_MS * 2);
    return () => window.clearTimeout(fallback);
  }, [closing, finish]);

  useEscapeToClose(dismiss, !closing);

  useEffect(() => {
    const previouslyFocused = document.activeElement as HTMLElement | null;
    const node = ref.current;
    const focusables = () =>
      node
        ? Array.from(node.querySelectorAll<HTMLElement>(FOCUSABLE)).filter((el) => el.offsetParent !== null)
        : [];

    const all = focusables();
    const initial =
      node?.querySelector<HTMLElement>('[data-autofocus]') ??
      all.find((el) => el.closest('[data-modal-body]')) ??
      all[0];
    initial?.focus();

    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Tab') return;
      const f = focusables();
      if (f.length === 0) return;
      const first = f[0];
      const last = f[f.length - 1];
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    };

    node?.addEventListener('keydown', onKeyDown);
    return () => {
      node?.removeEventListener('keydown', onKeyDown);
      previouslyFocused?.focus?.();
    };
  }, []);

  // Rendered at the document root rather than in place. The mobile navigation
  // drawer animates with `translate-x`, and a transformed ancestor becomes the
  // containing block for `position: fixed` — so any dialog opened from the
  // sidebar was laid out inside the drawer's 304px instead of over the screen.
  return createPortal(
    <div
      className={`${
        backdropClassName ??
        'fixed inset-0 bg-overlay/50 flex items-end justify-center z-50 sm:items-center sm:p-4'
      } ${closing ? 'backdrop-exit' : 'backdrop-enter'}`}
      onMouseDown={dismiss}
    >
      <div
        ref={ref}
        role="dialog"
        aria-modal="true"
        aria-label={label}
        data-qa={dataQa}
        onMouseDown={(e) => e.stopPropagation()}
        onAnimationEnd={(e) => {
          if (closing && e.target === e.currentTarget) finish();
        }}
        className={`${className} ${closing ? 'dialog-exit' : 'dialog-enter'}`}
      >
        <DismissContext.Provider value={dismiss}>{children}</DismissContext.Provider>
      </div>
    </div>,
    document.body,
  );
}
