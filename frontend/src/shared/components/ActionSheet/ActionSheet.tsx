import { useEffect, useState, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { useEscapeToClose } from '@/shared/hooks/useEscapeToClose';

interface Props {
  title?: string;
  onClose: () => void;
  children: ReactNode;
  dataQa?: string;
}

export function ActionSheet({ title, onClose, children, dataQa }: Props) {
  const [shown, setShown] = useState(false);
  useEscapeToClose(onClose);

  useEffect(() => {
    const frame = requestAnimationFrame(() => setShown(true));
    return () => cancelAnimationFrame(frame);
  }, []);

  return createPortal(
    <div className="fixed inset-0 z-60 flex items-end" role="presentation">
      <div
        className={`absolute inset-0 bg-overlay/60 transition-opacity ${shown ? 'opacity-100' : 'opacity-0'}`}
        onClick={onClose}
        aria-hidden
      />
      <div
        role="dialog"
        aria-modal="true"
        aria-label={title ?? 'Actions'}
        data-qa={dataQa}
        className={`relative w-full max-h-[80dvh] overflow-y-auto bg-surface border-t border-line rounded-t-2xl shadow-2xl pb-[max(0.75rem,env(safe-area-inset-bottom))] transition-transform duration-200 ${
          shown ? 'translate-y-0' : 'translate-y-full'
        }`}
      >
        <div className="flex justify-center pt-2.5 pb-1">
          <span className="w-10 h-1 rounded-full bg-line-strong" aria-hidden />
        </div>
        {children}
      </div>
    </div>,
    document.body,
  );
}
