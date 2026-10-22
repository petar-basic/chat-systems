import type { ReactNode } from 'react';
import { X } from 'lucide-react';
import { IconButton } from '../IconButton/IconButton';

interface PanelHeaderProps {
  title: string;
  icon?: ReactNode;
  badge?: ReactNode;
  actions?: ReactNode;
  onClose: () => void;
  closeLabel?: string;
}

export function PanelHeader({ title, icon, badge, actions, onClose, closeLabel }: PanelHeaderProps) {
  return (
    <div className="h-14 shrink-0 px-4 flex items-center justify-between gap-2 border-b border-line/50">
      <h2 className="flex min-w-0 items-center gap-2 font-semibold text-fg">
        {icon}
        <span className="truncate">{title}</span>
        {badge}
      </h2>
      <div className="-mr-1.5 flex items-center gap-1">
        {actions}
        <IconButton
          label={closeLabel ?? `Close ${title.toLowerCase()}`}
          icon={<X className="h-4 w-4" />}
          onClick={onClose}
        />
      </div>
    </div>
  );
}
