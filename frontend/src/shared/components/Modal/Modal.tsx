import { useEffect, useRef, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { useEscapeToClose } from '@/shared/hooks/useEscapeToClose';

interface ModalProps {
  title: string;
  onClose: () => void;
  children: ReactNode;
  className?: string;
  dataQa?: string;
}

const FOCUSABLE =
  'a[href],button:not([disabled]),textarea:not([disabled]),input:not([disabled]),select:not([disabled]),[tabindex]:not([tabindex="-1"])';

export function Modal({ title, onClose, children, className, dataQa }: ModalProps) {
  const ref = useRef<HTMLDivElement>(null);
  useEscapeToClose(onClose);

  useEffect(() => {
    const previouslyFocused = document.activeElement as HTMLElement | null;
    const node = ref.current;
    const focusables = () =>
      node
        ? Array.from(node.querySelectorAll<HTMLElement>(FOCUSABLE)).filter((el) => el.offsetParent !== null)
        : [];

    focusables()[0]?.focus();

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
      className="fixed inset-0 bg-overlay/50 flex items-end justify-center z-50 sm:items-center sm:p-4"
      onMouseDown={onClose}
    >
      <div
        ref={ref}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        data-qa={dataQa}
        onMouseDown={(e) => e.stopPropagation()}
        // A phone gets a sheet that reaches the bottom edge and can grow to the
        // full height; anything wider keeps the centred dialog.
        className={`max-h-[90dvh] overflow-y-auto pb-[env(safe-area-inset-bottom)] sm:pb-0 ${
          className ??
          'bg-surface border border-line rounded-t-2xl sm:rounded-2xl p-6 w-full sm:max-w-sm shadow-2xl'
        }`}
      >
        {children}
      </div>
    </div>,
    document.body,
  );
}
