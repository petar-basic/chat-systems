import type { ReactNode } from 'react';
import { X } from 'lucide-react';
import { IconButton } from '../IconButton/IconButton';

interface DialogHeaderProps {
  title: string;
  description?: ReactNode;
  onClose: () => void;
}

export function DialogHeader({ title, description, onClose }: DialogHeaderProps) {
  return (
    <div className="mb-4 flex items-start justify-between gap-4">
      <div className="min-w-0">
        <h2 className="text-lg font-semibold text-fg">{title}</h2>
        {description && <p className="mt-1 text-sm text-muted">{description}</p>}
      </div>
      <IconButton
        label="Close"
        icon={<X className="h-4 w-4" />}
        onClick={onClose}
        className="-mr-1.5 -mt-0.5"
      />
    </div>
  );
}

interface DialogFooterProps {
  children: ReactNode;
  start?: ReactNode;
}

export function DialogFooter({ children, start }: DialogFooterProps) {
  return (
    <div className="mt-5 flex items-center justify-end gap-2">
      {start && <div className="mr-auto min-w-0">{start}</div>}
      {children}
    </div>
  );
}
