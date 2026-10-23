import { Modal } from '@/shared/components/Modal/Modal';
import { isApplePlatform } from './shortcuts';

type Key = 'mod' | 'alt' | 'shift' | string;

interface Binding {
  label: string;
  keys: Key[][];
  note?: string;
}

const GROUPS: { title: string; bindings: Binding[] }[] = [
  {
    title: 'Getting around',
    bindings: [
      { label: 'Jump to a conversation', keys: [['mod', 'K']] },
      {
        label: 'Previous or next channel',
        keys: [
          ['alt', '↑'],
          ['alt', '↓'],
        ],
      },
      {
        label: 'Previous or next unread channel',
        keys: [
          ['alt', 'shift', '↑'],
          ['alt', 'shift', '↓'],
        ],
      },
      { label: 'All unreads', keys: [['alt', 'shift', 'A']] },
      { label: 'Keyboard shortcuts', keys: [['mod', '/']] },
    ],
  },
  {
    title: 'Reading',
    bindings: [
      { label: 'Mark the channel read', keys: [['Esc']] },
      { label: 'Mark everything read', keys: [['shift', 'Esc']] },
    ],
  },
  {
    title: 'Messages',
    bindings: [
      { label: 'Edit your last message', keys: [['↑']], note: 'in an empty message box' },
      { label: 'Move between messages', keys: [['↑'], ['↓']], note: 'on a focused message' },
      { label: 'Edit', keys: [['E']] },
      { label: 'Open the thread', keys: [['T'], ['Enter']] },
      { label: 'Add a reaction', keys: [['R']] },
      { label: 'Save for later', keys: [['A']] },
      { label: 'Pin or unpin', keys: [['P']] },
      { label: 'Back to the message box', keys: [['Esc']] },
    ],
  },
];

function keyName(key: Key, apple: boolean): string {
  if (key === 'mod') return apple ? '⌘' : 'Ctrl';
  if (key === 'alt') return apple ? '⌥' : 'Alt';
  if (key === 'shift') return apple ? '⇧' : 'Shift';
  return key;
}

export function KeyboardShortcutsDialog({ onClose }: { onClose: () => void }) {
  const apple = isApplePlatform();
  return (
    <Modal title="Keyboard shortcuts" onClose={onClose} dataQa="keyboard-shortcuts" size="lg">
      <div className="space-y-5">
        {GROUPS.map((group) => (
          <section key={group.title}>
            <h3 className="mb-2 text-xs font-semibold uppercase tracking-wide text-muted">{group.title}</h3>
            <dl className="divide-y divide-line">
              {group.bindings.map((binding) => (
                <div key={binding.label} className="flex items-center justify-between gap-4 py-1.5">
                  <dt className="text-sm text-fg-dim">
                    {binding.label}
                    {binding.note && <span className="ml-1 text-xs text-subtle">{binding.note}</span>}
                  </dt>
                  <dd className="flex shrink-0 items-center gap-2">
                    {binding.keys.map((combo, index) => (
                      <span key={index} className="flex items-center gap-1">
                        {index > 0 && <span className="text-xs text-subtle">or</span>}
                        {combo.map((key) => (
                          <kbd
                            key={key}
                            className="min-w-6 rounded border border-line-strong bg-raised px-1.5 py-0.5 text-center font-sans text-xs text-fg"
                          >
                            {keyName(key, apple)}
                          </kbd>
                        ))}
                      </span>
                    ))}
                  </dd>
                </div>
              ))}
            </dl>
          </section>
        ))}
      </div>
    </Modal>
  );
}
