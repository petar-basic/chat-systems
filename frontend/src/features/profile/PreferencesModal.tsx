import { useState } from 'react';
import { Bell, Palette, ShieldCheck, UserRound } from 'lucide-react';
import { SectionedModal, type ModalSection } from '@/shared/components/Modal/SectionedModal';
import {
  PushNotificationsPanel,
  EmailNotificationsPanel,
  KeywordsPanel,
  NotifyDefaultPanel,
  ScheduleEditor,
} from '@/features/notifications';
import ThemePicker from '@/features/profile/ThemePicker';
import TwoFactorPanel from '@/features/profile/TwoFactorPanel';
import AccountPanel from '@/features/profile/AccountPanel';
import { useWorkspaceStore } from '@/stores/workspace';

export type PreferencesSection = 'notifications' | 'appearance' | 'account' | 'security';

const SECTIONS: ModalSection<PreferencesSection>[] = [
  { key: 'notifications', label: 'Notifications', Icon: Bell },
  { key: 'appearance', label: 'Appearance', Icon: Palette },
  { key: 'account', label: 'Account', Icon: UserRound },
  { key: 'security', label: 'Security', Icon: ShieldCheck },
];

interface Props {
  initial?: PreferencesSection;
  onClose: () => void;
}

export function PreferencesModal({ initial = 'notifications', onClose }: Props) {
  const [active, setActive] = useState<PreferencesSection>(initial);
  const workspace = useWorkspaceStore((s) => s.currentWorkspace);

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
          {workspace && (
            <p className="text-sm text-muted" data-qa="notifications-scope">
              Except push on this device, these are your settings in{' '}
              <strong className="text-fg">{workspace.name}</strong>; each workspace has its own.
            </p>
          )}
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
      {active === 'account' && <AccountPanel />}
      {active === 'security' && <TwoFactorPanel />}
    </SectionedModal>
  );
}
