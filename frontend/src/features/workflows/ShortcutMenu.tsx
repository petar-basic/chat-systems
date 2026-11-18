import { useRef, useState } from 'react';
import { Zap } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useGlobalShortcuts, useRunAppShortcut, type GlobalShortcut } from '@/features/apps';
import { useWorkspaceStore } from '@/stores/workspace';
import { Avatar } from '@/shared/components/Avatar/Avatar';
import { AnchoredPopover } from '@/shared/components/Popover/AnchoredPopover';
import { toast } from '@/shared/components/Toast';
import { toUserMessage } from '@/lib/errors';
import { MenuItem, roveMenu } from '@/features/lists';
import { useStartWorkflow, useWorkflowShortcuts } from './useWorkflows';
import type { WorkflowShortcut } from './workflowModel';

interface Props {
  channelId: string;
  instanceUrl?: string;
}

export function ShortcutMenu({ channelId, instanceUrl }: Props) {
  const anchor = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  const { t } = useTranslation('messages');
  const shortcuts = useWorkflowShortcuts(channelId, instanceUrl).data ?? [];
  const start = useStartWorkflow(instanceUrl);
  const workspaceId = useWorkspaceStore((s) => s.currentWorkspace?.id);
  const appShortcuts = useGlobalShortcuts(workspaceId);
  const runApp = useRunAppShortcut();

  if (shortcuts.length === 0 && appShortcuts.length === 0) return null;

  const runAppShortcut = ({ shortcut }: GlobalShortcut) => {
    setOpen(false);
    runApp.mutate(
      { shortcutId: shortcut.id, channelId },
      {
        onError: (err) => toast.error(toUserMessage(err, t('appShortcuts.failed', { name: shortcut.name }))),
      },
    );
  };

  const run = async (shortcut: WorkflowShortcut) => {
    setOpen(false);
    try {
      await start.mutateAsync(shortcut.id);
      toast.success(`Started “${shortcut.name}”`);
    } catch (err) {
      toast.error(toUserMessage(err, `Couldn't start “${shortcut.name}”`));
    }
  };

  return (
    <>
      <button
        ref={anchor}
        type="button"
        onClick={() => setOpen((v) => !v)}
        aria-label={t('appShortcuts.heading')}
        data-tooltip={t('appShortcuts.heading')}
        aria-haspopup="menu"
        aria-expanded={open}
        data-qa="composer-workflows"
        className="p-1 text-muted hover:text-fg-soft transition cursor-pointer rounded hover:bg-raised/60"
      >
        <Zap className="w-3.5 h-3.5" />
      </button>
      {open && (
        <AnchoredPopover
          anchorRef={anchor}
          onClose={() => setOpen(false)}
          align="start"
          dataQa="composer-workflows-menu"
          className="w-64 bg-surface border border-line rounded-lg shadow-xl p-1"
        >
          <div role="menu" aria-label={t('appShortcuts.heading')} onKeyDown={roveMenu}>
            {shortcuts.map((shortcut, index) => (
              <MenuItem
                key={shortcut.id}
                first={index === 0}
                icon={<Zap className="h-4 w-4 shrink-0" />}
                label={shortcut.name}
                onSelect={() => void run(shortcut)}
                qa="composer-workflow"
              />
            ))}
            {appShortcuts.map((entry, index) => (
              <MenuItem
                key={entry.shortcut.id}
                first={shortcuts.length === 0 && index === 0}
                icon={
                  <Avatar
                    userId={entry.app.bot_user_id}
                    name={entry.app.name}
                    avatarUrl={entry.app.icon_url}
                    size="xs"
                  />
                }
                label={entry.shortcut.name}
                onSelect={() => runAppShortcut(entry)}
                qa="composer-app-shortcut"
              />
            ))}
          </div>
        </AnchoredPopover>
      )}
    </>
  );
}
