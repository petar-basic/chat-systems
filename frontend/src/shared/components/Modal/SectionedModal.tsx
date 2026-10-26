import { useId, useRef, type KeyboardEvent, type ReactNode } from 'react';
import type { LucideIcon } from 'lucide-react';
import { Modal, type ModalSize } from './Modal';

export interface ModalSection<K extends string> {
  key: K;
  label: string;
  Icon: LucideIcon;
}

const NEXT_KEYS: Record<string, number> = { ArrowDown: 1, ArrowRight: 1, ArrowUp: -1, ArrowLeft: -1 };

interface Props<K extends string> {
  title: string;
  sections: ModalSection<K>[];
  active: K;
  onSelect: (key: K) => void;
  onClose: () => void;
  size?: ModalSize;
  qaPrefix: string;
  children: ReactNode;
}

export function SectionedModal<K extends string>({
  title,
  sections,
  active,
  onSelect,
  onClose,
  size = 'lg',
  qaPrefix,
  children,
}: Props<K>) {
  const tabs = useRef<Record<string, HTMLButtonElement | null>>({});
  const id = useId();

  const select = (index: number) => {
    const section = sections[(index + sections.length) % sections.length];
    onSelect(section.key);
    tabs.current[section.key]?.focus();
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const current = sections.findIndex((s) => s.key === active);
    if (e.key in NEXT_KEYS) select(current + NEXT_KEYS[e.key]);
    else if (e.key === 'Home') select(0);
    else if (e.key === 'End') select(sections.length - 1);
    else return;
    e.preventDefault();
  };

  const section = sections.find((s) => s.key === active) ?? sections[0];

  return (
    <Modal title={title} onClose={onClose} size={size} dataQa={`${qaPrefix}-modal`}>
      <div className="grid min-h-80 gap-5 sm:grid-cols-[176px_1fr]">
        <div
          role="tablist"
          aria-label={title}
          aria-orientation="vertical"
          onKeyDown={onKeyDown}
          className="flex gap-1 max-sm:overflow-x-auto sm:flex-col"
        >
          {sections.map(({ key, label, Icon }) => {
            const selected = key === section.key;
            return (
              <button
                key={key}
                ref={(node) => {
                  tabs.current[key] = node;
                }}
                type="button"
                role="tab"
                id={`${id}-tab-${key}`}
                aria-selected={selected}
                aria-controls={`${id}-panel`}
                tabIndex={selected ? 0 : -1}
                onClick={() => onSelect(key)}
                data-qa={`${qaPrefix}-tab-${key}`}
                className={`flex shrink-0 cursor-pointer items-center gap-2.5 rounded-lg px-3 py-2 text-left text-sm transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-purple-500 ${
                  selected
                    ? 'bg-purple-600/15 font-medium text-fg'
                    : 'text-fg-dim hover:bg-raised/60 hover:text-fg'
                }`}
              >
                <Icon className={`h-4 w-4 ${selected ? 'text-accent' : 'text-muted'}`} />
                {label}
              </button>
            );
          })}
        </div>

        <section
          role="tabpanel"
          id={`${id}-panel`}
          aria-labelledby={`${id}-tab-${section.key}`}
          data-qa={`${qaPrefix}-panel-${section.key}`}
          className="min-w-0 space-y-6"
        >
          <h3 className="text-base font-semibold text-fg">{section.label}</h3>
          {children}
        </section>
      </div>
    </Modal>
  );
}
