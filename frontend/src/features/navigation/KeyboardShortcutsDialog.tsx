import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';
import { Modal } from '@/shared/components/Modal/Modal';
import { useDisplayStore } from '@/stores/display';
import { isApplePlatform, keyName, newLineKeys, sendKeys } from './shortcuts';

type Key = 'mod' | 'alt' | 'shift' | string;

interface Binding {
  id: string;
  label: string;
  keys: Key[][];
  note?: string;
}

const GROUPS = (
  t: TFunction<['messages', 'sidebar']>,
  sending: Binding[],
): { id: string; title: string; bindings: Binding[] }[] => [
  {
    id: 'gettingAround',
    title: t('shortcuts.groups.gettingAround'),
    bindings: [
      { id: 'jumpTo', label: t('shortcuts.jumpTo'), keys: [['mod', 'K']] },
      {
        id: 'channel',
        label: t('shortcuts.channel'),
        keys: [
          ['alt', '↑'],
          ['alt', '↓'],
        ],
      },
      {
        id: 'unreadChannel',
        label: t('shortcuts.unreadChannel'),
        keys: [
          ['alt', 'shift', '↑'],
          ['alt', 'shift', '↓'],
        ],
      },
      { id: 'allUnreads', label: t('sidebar:allUnreads'), keys: [['alt', 'shift', 'A']] },
      { id: 'shortcuts', label: t('shortcuts.title'), keys: [['mod', '/']] },
    ],
  },
  {
    id: 'reading',
    title: t('shortcuts.groups.reading'),
    bindings: [
      { id: 'markChannelRead', label: t('shortcuts.markChannelRead'), keys: [['Esc']] },
      { id: 'markAllRead', label: t('shortcuts.markAllRead'), keys: [['shift', 'Esc']] },
    ],
  },
  {
    id: 'writing',
    title: t('shortcuts.groups.writing'),
    bindings: sending,
  },
  {
    id: 'messages',
    title: t('shortcuts.groups.messages'),
    bindings: [
      {
        id: 'editLast',
        label: t('shortcuts.editLast'),
        keys: [['↑']],
        note: t('shortcuts.emptyComposer'),
      },
      {
        id: 'move',
        label: t('shortcuts.move'),
        keys: [['↑'], ['↓']],
        note: t('shortcuts.focusedMessage'),
      },
      { id: 'edit', label: t('actions.edit'), keys: [['E']] },
      { id: 'openThread', label: t('shortcuts.openThread'), keys: [['T'], ['Enter']] },
      { id: 'addReaction', label: t('shortcuts.addReaction'), keys: [['R']] },
      { id: 'save', label: t('actions.save'), keys: [['A']] },
      { id: 'pin', label: t('shortcuts.pin'), keys: [['P']] },
      { id: 'composer', label: t('shortcuts.composer'), keys: [['Esc']] },
    ],
  },
];

export function KeyboardShortcutsDialog({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation(['messages', 'sidebar']);
  const apple = isApplePlatform();
  const enterKey = useDisplayStore((s) => s.enterKey);
  const sending: Binding[] = [
    { id: 'send', label: t('shortcuts.send'), keys: [sendKeys(enterKey)] },
    { id: 'newLine', label: t('shortcuts.newLine'), keys: [newLineKeys(enterKey)] },
  ];
  return (
    <Modal title={t('shortcuts.title')} onClose={onClose} dataQa="keyboard-shortcuts" size="lg">
      <div className="space-y-5">
        {GROUPS(t, sending).map((group) => (
          <section key={group.id}>
            <h3 className="mb-2 text-xs font-semibold uppercase tracking-wide text-muted">{group.title}</h3>
            <dl className="divide-y divide-line">
              {group.bindings.map((binding) => (
                <div key={binding.id} className="flex items-center justify-between gap-4 py-1.5">
                  <dt className="text-sm text-fg-dim">
                    {binding.label}
                    {binding.note && <span className="ml-1 text-xs text-subtle">{binding.note}</span>}
                  </dt>
                  <dd className="flex shrink-0 items-center gap-2">
                    {binding.keys.map((combo, index) => (
                      <span key={index} className="flex items-center gap-1">
                        {index > 0 && <span className="text-xs text-subtle">{t('shortcuts.or')}</span>}
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
