import { useState } from 'react';
import {
  Bell,
  CheckCheck,
  MessageSquare,
  Palette,
  ShieldCheck,
  SlidersHorizontal,
  UserRound,
} from 'lucide-react';
import { SectionedModal, type ModalSection } from '@/shared/components/Modal/SectionedModal';
import {
  PushNotificationsPanel,
  DesktopNotificationsPanel,
  EmailNotificationsPanel,
  KeywordsPanel,
  NotifyDefaultPanel,
  ScheduleEditor,
} from '@/features/notifications';
import ThemePicker from '@/features/profile/ThemePicker';
import WorkspaceThemePicker from '@/features/profile/WorkspaceThemePicker';
import LanguagePicker from '@/features/profile/LanguagePicker';
import TwoFactorPanel from '@/features/profile/TwoFactorPanel';
import MessagesMediaPanel from '@/features/profile/MessagesMediaPanel';
import AccountPanel from '@/features/profile/AccountPanel';
import MarkAsReadPanel from '@/features/profile/MarkAsReadPanel';
import AdvancedPanel from '@/features/profile/AdvancedPanel';
import { useWorkspaceStore } from '@/stores/workspace';

export type PreferencesSection =
  'notifications' | 'appearance' | 'messages' | 'read' | 'account' | 'security' | 'advanced';

const SECTIONS: ModalSection<PreferencesSection>[] = [
  { key: 'notifications', label: 'Notifications', Icon: Bell },
  { key: 'appearance', label: 'Appearance', Icon: Palette },
  { key: 'messages', label: 'Messages & media', Icon: MessageSquare },
  { key: 'read', label: 'Mark as read', Icon: CheckCheck },
  { key: 'account', label: 'Account', Icon: UserRound },
  { key: 'security', label: 'Security', Icon: ShieldCheck },
  { key: 'advanced', label: 'Advanced', Icon: SlidersHorizontal },
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
              Except push and desktop notifications on this device, these are your settings in{' '}
              <strong className="text-fg">{workspace.name}</strong>; each workspace has its own.
            </p>
          )}
          <NotifyDefaultPanel />
          <KeywordsPanel />
          <ScheduleEditor />
          <PushNotificationsPanel />
          <DesktopNotificationsPanel />
          <EmailNotificationsPanel />
        </>
      )}
      {active === 'appearance' && (
        <div>
          <p className="mb-3 text-sm text-muted">
            Pick a theme and a language, or follow your system and browser settings.
          </p>
          <ThemePicker />
          <WorkspaceThemePicker />
          <LanguagePicker />
        </div>
      )}
      {active === 'messages' && <MessagesMediaPanel />}
      {active === 'account' && <AccountPanel />}
      {active === 'read' && <MarkAsReadPanel />}
      {active === 'security' && <TwoFactorPanel />}
      {active === 'advanced' && <AdvancedPanel />}
    </SectionedModal>
  );
}
