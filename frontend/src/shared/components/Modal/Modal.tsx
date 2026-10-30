import { useCallback, useEffect, useRef, useState, type FormEvent, type ReactNode } from 'react';
import { X } from 'lucide-react';
import { Button } from '../Button/Button';
import { IconButton } from '../IconButton/IconButton';
import { Overlay } from './Overlay';
import { useDismiss } from './dismiss';

export type ModalSize = 'sm' | 'md' | 'lg' | 'tall' | 'full';

const WIDTHS: Record<ModalSize, string> = {
  sm: 'sm:max-w-[400px]',
  md: 'sm:max-w-[520px]',
  lg: 'sm:max-w-[720px]',
  tall: 'sm:h-[min(44rem,90dvh)] sm:max-w-[720px]',
  full: 'h-[90dvh] sm:max-w-[880px]',
};

interface ModalProps {
  title: string;
  description?: ReactNode;
  headerActions?: ReactNode;
  onClose: () => void;
  size?: ModalSize;
  actions?: ReactNode;
  footerStart?: ReactNode;
  dismissLabel?: string;
  onSubmit?: (e: FormEvent<HTMLFormElement>) => void;
  dataQa?: string;
  children: ReactNode;
}

export function Modal({
  title,
  description,
  headerActions,
  onClose,
  size = 'md',
  actions,
  footerStart,
  dismissLabel,
  onSubmit,
  dataQa,
  children,
}: ModalProps) {
  const hasFooter = !!(actions || footerStart || dismissLabel);
  const content = (
    <ModalLayout
      title={title}
      description={description}
      headerActions={headerActions}
      footer={hasFooter && <ModalFooter start={footerStart} dismissLabel={dismissLabel} actions={actions} />}
    >
      {children}
    </ModalLayout>
  );

  return (
    <Overlay
      label={title}
      onClose={onClose}
      dataQa={dataQa}
      className={`flex max-h-[90dvh] w-full flex-col rounded-t-2xl border border-line bg-surface pb-[env(safe-area-inset-bottom)] shadow-2xl sm:rounded-2xl sm:pb-0 ${WIDTHS[size]}`}
    >
      {onSubmit ? (
        <form onSubmit={onSubmit} noValidate className="flex min-h-0 flex-1 flex-col">
          {content}
        </form>
      ) : (
        content
      )}
    </Overlay>
  );
}

function ModalLayout({
  title,
  description,
  headerActions,
  footer,
  children,
}: {
  title: string;
  description?: ReactNode;
  headerActions?: ReactNode;
  footer: ReactNode;
  children: ReactNode;
}) {
  const dismiss = useDismiss();
  const body = useRef<HTMLDivElement>(null);
  const [edges, setEdges] = useState({ above: false, below: false });

  const measure = useCallback(() => {
    const node = body.current;
    if (!node) return;
    const above = node.scrollTop > 0;
    const below = node.scrollTop + node.clientHeight < node.scrollHeight - 1;
    setEdges((prev) => (prev.above === above && prev.below === below ? prev : { above, below }));
  }, []);

  useEffect(() => {
    const node = body.current;
    if (!node || typeof ResizeObserver === 'undefined') return undefined;
    const observer = new ResizeObserver(measure);
    observer.observe(node);
    for (const child of Array.from(node.children)) observer.observe(child);
    return () => observer.disconnect();
  }, [measure]);

  return (
    <>
      <header
        className={`flex shrink-0 items-start justify-between gap-4 border-b px-6 pt-5 pb-4 transition-colors ${
          edges.above ? 'border-line/60' : 'border-transparent'
        }`}
      >
        <div className="min-w-0">
          <h2 className="text-lg font-semibold text-fg">{title}</h2>
          {description && <p className="mt-1 text-sm text-muted">{description}</p>}
        </div>
        <div className="-mr-1.5 -mt-0.5 flex shrink-0 items-center gap-2">
          {headerActions}
          <IconButton label="Close" icon={<X className="h-4 w-4" />} onClick={dismiss} />
        </div>
      </header>
      <div
        ref={body}
        onScroll={measure}
        data-modal-body
        className={`min-h-0 flex-1 overflow-y-auto px-6 py-1 ${footer ? '' : 'pb-6'}`}
      >
        {children}
      </div>
      {footer && (
        <footer
          className={`shrink-0 border-t px-6 pt-4 pb-5 transition-colors ${
            edges.below ? 'border-line/60' : 'border-transparent'
          }`}
        >
          {footer}
        </footer>
      )}
    </>
  );
}

function ModalFooter({
  start,
  dismissLabel,
  actions,
}: {
  start?: ReactNode;
  dismissLabel?: string;
  actions?: ReactNode;
}) {
  const dismiss = useDismiss();
  return (
    <div className="flex items-center justify-end gap-2">
      {start && <div className="mr-auto min-w-0">{start}</div>}
      {dismissLabel && (
        <Button variant="ghost" onClick={dismiss}>
          {dismissLabel}
        </Button>
      )}
      {actions}
    </div>
  );
}
