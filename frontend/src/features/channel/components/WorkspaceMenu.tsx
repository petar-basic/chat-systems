import { useRef, useState } from 'react';
import { ChevronDown, Settings, ShieldCheck, UserPlus, Workflow } from 'lucide-react';
import { useOnClickOutside } from '@/shared/hooks/useOnClickOutside';
import { useEscapeToClose } from '@/shared/hooks/useEscapeToClose';

interface Props {
  workspaceName: string | undefined;
  isWorkspaceAdmin: boolean;
  isInstanceAdmin: boolean;
  canUseWorkflows: boolean;
  onOpenSettings: () => void;
  onInvite: () => void;
  onOpenWorkflows: () => void;
  onOpenInstanceAdmin: () => void;
}

const ITEM =
  'w-full px-4 py-2 text-left text-sm text-fg-dim hover:bg-raised flex items-center gap-2 cursor-pointer';

export function WorkspaceMenu({
  workspaceName,
  isWorkspaceAdmin,
  isInstanceAdmin,
  canUseWorkflows,
  onOpenSettings,
  onInvite,
  onOpenWorkflows,
  onOpenInstanceAdmin,
}: Props) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useOnClickOutside(ref, () => setOpen(false), open);
  useEscapeToClose(() => setOpen(false), open);

  const pick = (action: () => void) => () => {
    action();
    setOpen(false);
  };

  return (
    <div className="relative" ref={ref}>
      <button
        onClick={() => setOpen(!open)}
        aria-expanded={open}
        data-qa="workspace-menu"
        className="w-full px-4 py-3 flex items-center justify-between border-b border-line/50 hover:bg-raised/30 transition cursor-pointer"
      >
        <span className="font-semibold text-fg truncate">{workspaceName || 'Select workspace'}</span>
        <ChevronDown className="w-4 h-4 text-muted" />
      </button>
      {open && (
        <div className="absolute top-full left-0 right-0 bg-surface border border-line rounded-b-lg shadow-xl z-30">
          {isWorkspaceAdmin && (
            <button className={ITEM} data-qa="open-invite" onClick={pick(onInvite)}>
              <UserPlus className="w-4 h-4" /> Invite people
            </button>
          )}
          {canUseWorkflows && (
            <button className={ITEM} data-qa="open-workflows" onClick={pick(onOpenWorkflows)}>
              <Workflow className="w-4 h-4" /> Workflows
            </button>
          )}
          <button className={ITEM} data-qa="open-workspace-settings" onClick={pick(onOpenSettings)}>
            <Settings className="w-4 h-4" /> Workspace settings
          </button>
          {isInstanceAdmin && (
            <button
              className="w-full px-4 py-2 text-left text-sm text-accent-soft hover:bg-raised flex items-center gap-2 cursor-pointer border-t border-line"
              onClick={pick(onOpenInstanceAdmin)}
            >
              <ShieldCheck className="w-4 h-4" /> Instance admin
            </button>
          )}
        </div>
      )}
    </div>
  );
}
