import { useState } from 'react';
import { Bell, Palette, ShieldCheck } from 'lucide-react';
import { SectionedModal, type ModalSection } from '@/shared/components/Modal/SectionedModal';
import PushNotificationsPanel from '@/features/notifications/PushNotificationsPanel';
import EmailNotificationsPanel from '@/features/notifications/EmailNotificationsPanel';
import ThemePicker from '@/features/profile/ThemePicker';
import TwoFactorPanel from '@/features/profile/TwoFactorPanel';
import { KeywordsPanel, NotifyDefaultPanel, ScheduleEditor } from '@/features/notifications';

export type PreferencesSection = 'notifications' | 'appearance' | 'security';

const SECTIONS: ModalSection<PreferencesSection>[] = [
  { key: 'notifications', label: 'Notifications', Icon: Bell },
  { key: 'appearance', label: 'Appearance', Icon: Palette },
  { key: 'security', label: 'Security', Icon: ShieldCheck },
];

interface Props {
  initial?: PreferencesSection;
  onClose: () => void;
}

export function PreferencesModal({ initial = 'notifications', onClose }: Props) {
  const [active, setActive] = useState<PreferencesSection>(initial);

  return (
    <SectionedModal
      title="Preferences"
      sections={SECTIONS}
      active={active}
      onSelect={setActive}
      onClose={onClose}
      qaPrefix="preferences"
    >
      {active === 'notifications' && (
        <>
          <NotifyDefaultPanel />
          <KeywordsPanel />
          <ScheduleEditor />
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
    </SectionedModal>
  );
}
