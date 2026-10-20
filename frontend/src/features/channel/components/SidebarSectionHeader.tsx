import { ChevronRight, Plus } from 'lucide-react';
import type { SidebarSection } from '../hooks/useCollapsedSections';

interface Props {
  section: SidebarSection;
  label: string;
  collapsed: boolean;
  onToggle: (section: SidebarSection) => void;
  action?: { label: string; onClick: () => void };
  className?: string;
}

export function SidebarSectionHeader({ section, label, collapsed, onToggle, action, className = '' }: Props) {
  return (
    <div className={`px-3 mb-1 flex items-center justify-between ${className}`}>
      <button
        onClick={() => onToggle(section)}
        aria-expanded={!collapsed}
        data-qa={`toggle-section-${section}`}
        className="flex items-center gap-1 text-xs font-semibold text-muted uppercase tracking-wider hover:text-fg-soft transition cursor-pointer"
      >
        <ChevronRight className={`w-3 h-3 transition-transform ${collapsed ? '' : 'rotate-90'}`} />
        {label}
      </button>
      {action && (
        <button
          onClick={action.onClick}
          aria-label={action.label}
          title={action.label}
          className="text-muted hover:text-fg transition cursor-pointer"
        >
          <Plus className="w-4 h-4" />
        </button>
      )}
    </div>
  );
}
