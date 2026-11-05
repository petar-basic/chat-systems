import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Bell,
  CheckCheck,
  Globe,
  MessageSquare,
  Palette,
  ShieldCheck,
  SlidersHorizontal,
  UserRound,
} from 'lucide-react';
import { SectionedModal, type ModalSection } from '@/shared/components/Modal/SectionedModal';
import { Trans } from '@/shared/components/Trans/Trans';
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
import TextPicker from '@/features/profile/TextPicker';
import RegionPanel from '@/features/profile/RegionPanel';
import TwoFactorPanel from '@/features/profile/TwoFactorPanel';
import PasskeysPanel from '@/features/profile/PasskeysPanel';
import MessagesMediaPanel from '@/features/profile/MessagesMediaPanel';
import AccountPanel from '@/features/profile/AccountPanel';
import MarkAsReadPanel from '@/features/profile/MarkAsReadPanel';
import AdvancedPanel from '@/features/profile/AdvancedPanel';
import { useWorkspaceStore } from '@/stores/workspace';

export type PreferencesSection =
  'notifications' | 'appearance' | 'messages' | 'region' | 'read' | 'account' | 'security' | 'advanced';

const SECTIONS: Omit<ModalSection<PreferencesSection>, 'label'>[] = [
  { key: 'notifications', Icon: Bell },
  { key: 'appearance', Icon: Palette },
  { key: 'messages', Icon: MessageSquare },
  { key: 'region', Icon: Globe },
  { key: 'read', Icon: CheckCheck },
  { key: 'account', Icon: UserRound },
  { key: 'security', Icon: ShieldCheck },
  { key: 'advanced', Icon: SlidersHorizontal },
];

interface Props {
  initial?: PreferencesSection;
  onClose: () => void;
}

export function PreferencesModal({ initial = 'notifications', onClose }: Props) {
  const [active, setActive] = useState<PreferencesSection>(initial);
  const workspace = useWorkspaceStore((s) => s.currentWorkspace);
  const { t } = useTranslation('profile');
  const sections = SECTIONS.map((section) => ({
    ...section,
    label: t(`preferences.sections.${section.key}`),
  }));

  return (
    <SectionedModal
      title={t('preferences.title')}
      sections={sections}
      active={active}
      onSelect={setActive}
      onClose={onClose}
      qaPrefix="preferences"
    >
      {active === 'notifications' && (
        <>
          {workspace && (
            <p className="text-sm text-muted" data-qa="notifications-scope">
              <Trans
                t={t}
                i18nKey="preferences.notificationsScope"
                values={{ name: workspace.name }}
                components={{ workspace: <strong className="text-fg" /> }}
              />
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
          <p className="mb-3 text-sm text-muted">{t('preferences.appearanceIntro')}</p>
          <ThemePicker />
          <WorkspaceThemePicker />
          <TextPicker />
        </div>
      )}
      {active === 'messages' && <MessagesMediaPanel />}
      {active === 'region' && <RegionPanel />}
      {active === 'account' && <AccountPanel />}
      {active === 'read' && <MarkAsReadPanel />}
      {active === 'security' && (
        <>
          <TwoFactorPanel />
          <PasskeysPanel />
        </>
      )}
      {active === 'advanced' && <AdvancedPanel />}
    </SectionedModal>
  );
}
