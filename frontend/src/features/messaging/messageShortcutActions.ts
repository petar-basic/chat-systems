import { useTranslation } from 'react-i18next';
import { Zap } from 'lucide-react';
import { useMessageShortcuts, useRunAppShortcut, type AppShortcut } from '@/features/apps';
import { toast } from '@/shared/components/Toast';
import { toUserMessage } from '@/lib/errors';
import type { Message } from '@/stores/workspace';
import type { SheetAction } from './MessageActionSheet';

export function useShortcutsOn(message: Message | undefined) {
  const { t } = useTranslation('messages');
  const usable = message && !message.pending ? message : undefined;
  const listed = useMessageShortcuts(usable?.channel_id).data;
  const { mutateAsync } = useRunAppShortcut();
  const run = (shortcut: AppShortcut) => {
    if (!usable) return;
    mutateAsync({ shortcutId: shortcut.id, messageId: usable.id }).catch((err) =>
      toast.error(toUserMessage(err, t('appShortcuts.failed', { name: shortcut.name }))),
    );
  };
  return { shortcuts: usable ? (listed ?? []) : [], run };
}

export function useMessageShortcutActions(message: Message | undefined): SheetAction[] {
  const { shortcuts, run } = useShortcutsOn(message);
  return shortcuts.map((shortcut) => ({
    key: `shortcut-${shortcut.id}`,
    label: shortcut.name,
    Icon: Zap,
    onSelect: () => run(shortcut),
  }));
}
