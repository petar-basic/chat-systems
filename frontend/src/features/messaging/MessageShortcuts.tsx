import { useTranslation } from 'react-i18next';
import { Zap } from 'lucide-react';
import type { Message } from '@/stores/workspace';
import { useShortcutsOn } from './messageShortcutActions';

interface Props {
  message: Message;
  onDone: () => void;
}

export function MessageShortcutItems({ message, onDone }: Props) {
  const { t } = useTranslation('messages');
  const { shortcuts, run } = useShortcutsOn(message);
  if (shortcuts.length === 0) return null;
  return (
    <div className="border-t border-line/60 py-1" data-qa="message-shortcuts">
      <div className="px-3 py-1 text-2xs uppercase tracking-wide text-subtle">
        {t('appShortcuts.heading')}
      </div>
      {shortcuts.map((shortcut) => (
        <button
          key={shortcut.id}
          onClick={() => {
            onDone();
            run(shortcut);
          }}
          data-qa="message-action-shortcut"
          className="w-full px-3 py-1.5 text-left text-sm text-fg-dim hover:bg-raised flex items-center gap-2"
        >
          <Zap className="w-3.5 h-3.5 shrink-0" /> <span className="truncate">{shortcut.name}</span>
        </button>
      ))}
    </div>
  );
}
