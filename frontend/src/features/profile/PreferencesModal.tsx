import { useId, useRef, useState, type KeyboardEvent } from 'react';
import { Bell, Palette, ShieldCheck } from 'lucide-react';
import { Modal } from '@/shared/components/Modal/Modal';
import PushNotificationsPanel from '@/components/PushNotificationsPanel';
import EmailNotificationsPanel from '@/components/EmailNotificationsPanel';
import ThemePicker from '@/components/ThemePicker';
import TwoFactorPanel from '@/components/TwoFactorPanel';

export type PreferencesSection = 'notifications' | 'appearance' | 'security';

const SECTIONS: { key: PreferencesSection; label: string; Icon: typeof Bell }[] = [
  { key: 'notifications', label: 'Notifications', Icon: Bell },
  { key: 'appearance', label: 'Appearance', Icon: Palette },
  { key: 'security', label: 'Security', Icon: ShieldCheck },
];

const NEXT_KEYS: Record<string, number> = { ArrowDown: 1, ArrowRight: 1, ArrowUp: -1, ArrowLeft: -1 };

interface Props {
  initial?: PreferencesSection;
  onClose: () => void;
}

export function PreferencesModal({ initial = 'notifications', onClose }: Props) {
  const [active, setActive] = useState<PreferencesSection>(initial);
  const tabs = useRef<Record<string, HTMLButtonElement | null>>({});
  const id = useId();

  const select = (index: number) => {
    const section = SECTIONS[(index + SECTIONS.length) % SECTIONS.length];
    setActive(section.key);
    tabs.current[section.key]?.focus();
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const current = SECTIONS.findIndex((s) => s.key === active);
    if (e.key in NEXT_KEYS) select(current + NEXT_KEYS[e.key]);
    else if (e.key === 'Home') select(0);
    else if (e.key === 'End') select(SECTIONS.length - 1);
    else return;
    e.preventDefault();
  };

  const section = SECTIONS.find((s) => s.key === active) ?? SECTIONS[0];

  return (
    <Modal title="Preferences" onClose={onClose} size="lg" dataQa="preferences-modal">
      <div className="grid min-h-80 gap-5 sm:grid-cols-[176px_1fr]">
        <div
          role="tablist"
          aria-label="Preferences"
          aria-orientation="vertical"
          onKeyDown={onKeyDown}
          className="flex gap-1 max-sm:overflow-x-auto sm:flex-col"
        >
          {SECTIONS.map(({ key, label, Icon }) => {
            const selected = key === active;
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
                onClick={() => setActive(key)}
                data-qa={`preferences-tab-${key}`}
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
          aria-labelledby={`${id}-tab-${active}`}
          data-qa={`preferences-panel-${active}`}
          className="min-w-0 space-y-6"
        >
          <h3 className="text-base font-semibold text-fg">{section.label}</h3>
          {active === 'notifications' && (
            <>
              <PushNotificationsPanel />
              <EmailNotificationsPanel />
            </>
          )}
          {active === 'appearance' && (
            <div>
              <p className="mb-3 text-sm text-muted">Pick a theme, or follow your system setting.</p>
              <ThemePicker />
            </div>
          )}
          {active === 'security' && <TwoFactorPanel />}
        </section>
      </div>
    </Modal>
  );
}
