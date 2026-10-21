import { useRef, useState, type ReactNode } from 'react';
import { ChevronRight, MoreHorizontal } from 'lucide-react';
import { AnchoredPopover } from '@/shared/components/Popover/AnchoredPopover';
import { SectionNameInput } from './SectionNameInput';
import { acceptsChannelDrop } from './channelDrag';

interface Props {
  id: string;
  name: string;
  collapsed: boolean;
  onToggle: () => void;
  onRename: (name: string) => void;
  onDelete: () => void;
  onMoveUp?: () => void;
  onMoveDown?: () => void;
  onDropChannel: (channelId: string) => void;
  children: ReactNode;
}

export function SidebarCustomSection({
  id,
  name,
  collapsed,
  onToggle,
  onRename,
  onDelete,
  onMoveUp,
  onMoveDown,
  onDropChannel,
  children,
}: Props) {
  const [renaming, setRenaming] = useState(false);
  const [menuOpen, setMenuOpen] = useState(false);
  const menuAnchor = useRef<HTMLButtonElement>(null);

  const item = (label: string, action: () => void, danger = false) => (
    <button
      type="button"
      onClick={() => {
        setMenuOpen(false);
        action();
      }}
      className={`w-full px-3 py-1.5 text-left text-sm hover:bg-raised ${danger ? 'text-danger' : 'text-fg-dim'}`}
    >
      {label}
    </button>
  );

  return (
    <div
      className="mt-4"
      data-qa="sidebar-custom-section"
      data-section-id={id}
      {...acceptsChannelDrop(onDropChannel)}
    >
      <div className="group/section px-3 mb-1 flex items-center justify-between gap-1">
        {renaming ? (
          <SectionNameInput
            initial={name}
            label="Section name"
            onSubmit={(next) => {
              setRenaming(false);
              if (next !== name) onRename(next);
            }}
            onCancel={() => setRenaming(false)}
          />
        ) : (
          <button
            onClick={onToggle}
            aria-expanded={!collapsed}
            className="flex min-w-0 items-center gap-1 text-xs font-semibold text-muted uppercase tracking-wider hover:text-fg-soft transition cursor-pointer"
          >
            <ChevronRight
              className={`w-3 h-3 shrink-0 transition-transform ${collapsed ? '' : 'rotate-90'}`}
            />
            <span className="truncate">{name}</span>
          </button>
        )}
        {!renaming && (
          <button
            ref={menuAnchor}
            type="button"
            onClick={() => setMenuOpen((open) => !open)}
            aria-label={`Options for ${name}`}
            aria-expanded={menuOpen}
            data-qa="sidebar-section-menu"
            className="p-0.5 rounded text-muted opacity-0 group-hover/section:opacity-100 focus-visible:opacity-100 hover:text-fg hover:bg-raised"
          >
            <MoreHorizontal className="w-4 h-4" />
          </button>
        )}
        {menuOpen && (
          <AnchoredPopover
            anchorRef={menuAnchor}
            onClose={() => setMenuOpen(false)}
            className="w-44 bg-surface border border-line rounded-lg shadow-xl py-1"
          >
            {item('Rename', () => setRenaming(true))}
            {onMoveUp && item('Move up', onMoveUp)}
            {onMoveDown && item('Move down', onMoveDown)}
            {item('Delete section', onDelete, true)}
          </AnchoredPopover>
        )}
      </div>
      {!collapsed && children}
    </div>
  );
}
