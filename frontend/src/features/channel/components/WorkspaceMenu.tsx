import { useRef, useState } from 'react';
import { ChevronDown, Plug, ScrollText, Settings, ShieldCheck, Smile, Upload, Users } from 'lucide-react';
import { useOnClickOutside } from '@/shared/hooks/useOnClickOutside';
import { useEscapeToClose } from '@/shared/hooks/useEscapeToClose';

interface Props {
  workspaceName: string | undefined;
  isWorkspaceAdmin: boolean;
  isInstanceAdmin: boolean;
  onOpenMembers: () => void;
  onOpenSettings: () => void;
  onOpenIntegrations: () => void;
  onOpenSlackImport: () => void;
  onOpenCustomEmoji: () => void;
  onOpenUserGroups: () => void;
  onOpenAuditLog: () => void;
  onOpenInstanceAdmin: () => void;
}

const ITEM =
  'w-full px-4 py-2 text-left text-sm text-fg-dim hover:bg-raised flex items-center gap-2 cursor-pointer';

export function WorkspaceMenu({
  workspaceName,
  isWorkspaceAdmin,
  isInstanceAdmin,
  onOpenMembers,
  onOpenSettings,
  onOpenIntegrations,
  onOpenSlackImport,
  onOpenCustomEmoji,
  onOpenUserGroups,
  onOpenAuditLog,
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
        className="w-full px-4 py-3 flex items-center justify-between border-b border-line/50 hover:bg-raised/30 transition cursor-pointer"
      >
        <span className="font-semibold text-fg truncate">{workspaceName || 'Select workspace'}</span>
        <ChevronDown className="w-4 h-4 text-muted" />
      </button>
      {open && (
        <div className="absolute top-full left-0 right-0 bg-surface border border-line rounded-b-lg shadow-xl z-30">
          <button className={ITEM} onClick={pick(onOpenMembers)}>
            <Users className="w-4 h-4" /> Members
          </button>
          <button className={ITEM} onClick={pick(onOpenSettings)}>
            <Settings className="w-4 h-4" /> Settings
          </button>
          {isWorkspaceAdmin && (
            <button className={ITEM} data-qa="open-integrations" onClick={pick(onOpenIntegrations)}>
              <Plug className="w-4 h-4" /> Integrations
            </button>
          )}
          {isWorkspaceAdmin && (
            <button className={ITEM} data-qa="open-slack-import" onClick={pick(onOpenSlackImport)}>
              <Upload className="w-4 h-4" /> Slack import
            </button>
          )}
          <button className={ITEM} data-qa="open-custom-emoji" onClick={pick(onOpenCustomEmoji)}>
            <Smile className="w-4 h-4" /> Custom emoji
          </button>
          <button className={ITEM} data-qa="open-user-groups" onClick={pick(onOpenUserGroups)}>
            <Users className="w-4 h-4" /> User groups
          </button>
          {isWorkspaceAdmin && (
            <button className={ITEM} data-qa="open-audit-log" onClick={pick(onOpenAuditLog)}>
              <ScrollText className="w-4 h-4" /> Audit log
            </button>
          )}
          {isInstanceAdmin && (
            <button
              className="w-full px-4 py-2 text-left text-sm text-accent-soft hover:bg-raised flex items-center gap-2 cursor-pointer border-t border-line"
              onClick={pick(onOpenInstanceAdmin)}
            >
              <ShieldCheck className="w-4 h-4" /> Instance Admin
            </button>
          )}
        </div>
      )}
    </div>
  );
}
