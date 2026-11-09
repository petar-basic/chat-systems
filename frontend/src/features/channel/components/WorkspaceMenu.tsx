import { useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ChevronDown, LogOut, Settings, ShieldCheck, UserPlus, Workflow } from 'lucide-react';
import { useOnClickOutside } from '@/shared/hooks/useOnClickOutside';
import { useEscapeToClose } from '@/shared/hooks/useEscapeToClose';

interface Props {
  workspaceName: string | undefined;
  isWorkspaceAdmin: boolean;
  isInstanceAdmin: boolean;
  canUseWorkflows: boolean;
  canLeave: boolean;
  onOpenSettings: () => void;
  onInvite: () => void;
  onOpenWorkflows: () => void;
  onOpenInstanceAdmin: () => void;
  onLeave: () => void;
}

const ITEM =
  'w-full px-4 py-2 text-left text-sm text-fg-dim hover:bg-raised flex items-center gap-2 cursor-pointer';

export function WorkspaceMenu({
  workspaceName,
  isWorkspaceAdmin,
  isInstanceAdmin,
  canUseWorkflows,
  canLeave,
  onOpenSettings,
  onInvite,
  onOpenWorkflows,
  onOpenInstanceAdmin,
  onLeave,
}: Props) {
  const { t } = useTranslation('sidebar');
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
        <span className="font-semibold text-fg truncate">{workspaceName || t('workspace.select')}</span>
        <ChevronDown className="w-4 h-4 text-muted" />
      </button>
      {open && (
        <div className="unthemed text-fg absolute top-full left-0 right-0 bg-surface border border-line rounded-b-lg shadow-xl z-30">
          {isWorkspaceAdmin && (
            <button className={ITEM} data-qa="open-invite" onClick={pick(onInvite)}>
              <UserPlus className="w-4 h-4" /> {t('workspace.invite')}
            </button>
          )}
          {canUseWorkflows && (
            <button className={ITEM} data-qa="open-workflows" onClick={pick(onOpenWorkflows)}>
              <Workflow className="w-4 h-4" /> {t('workspace.workflows')}
            </button>
          )}
          <button className={ITEM} data-qa="open-workspace-settings" onClick={pick(onOpenSettings)}>
            <Settings className="w-4 h-4" /> {t('workspace.settings')}
          </button>
          {canLeave && (
            <button className={ITEM} data-qa="leave-workspace" onClick={pick(onLeave)}>
              <LogOut className="w-4 h-4" /> {t('workspace.leave')}
            </button>
          )}
          {isInstanceAdmin && (
            <button
              className="w-full px-4 py-2 text-left text-sm text-accent-soft hover:bg-raised flex items-center gap-2 cursor-pointer border-t border-line"
              onClick={pick(onOpenInstanceAdmin)}
            >
              <ShieldCheck className="w-4 h-4" /> {t('workspace.instanceAdmin')}
            </button>
          )}
        </div>
      )}
    </div>
  );
}
