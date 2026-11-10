import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ChartColumn, Info, Plug, SlidersHorizontal } from 'lucide-react';
import { SectionedModal, type ModalSection } from '@/shared/components/Modal/SectionedModal';
import type { Channel } from '@/stores/workspace';
import type { AppRef } from '@/features/workspace';
import { AboutSection } from './AboutSection';
import { SettingsSection } from './SettingsSection';
import { IntegrationsSection } from './IntegrationsSection';
import { ActivitySection } from './ActivitySection';

export type ChannelDetailsTab = 'about' | 'settings' | 'activity' | 'integrations';

const ICONS = { about: Info, settings: SlidersHorizontal, activity: ChartColumn, integrations: Plug };

interface Props {
  channel: Channel;
  canModerate: boolean;
  initialTab?: ChannelDetailsTab;
  onOpenWorkflow?: (workflowId: string) => void;
  onOpenApp?: (app: AppRef) => void;
  onClose: () => void;
}

export function ChannelDetailsModal({
  channel,
  canModerate,
  initialTab = 'about',
  onOpenWorkflow,
  onOpenApp,
  onClose,
}: Props) {
  const { t } = useTranslation('channel');
  const keys: ChannelDetailsTab[] = canModerate
    ? ['about', 'settings', 'activity', 'integrations']
    : ['about', 'integrations'];
  const sections: ModalSection<ChannelDetailsTab>[] = keys.map((key) => ({
    key,
    label: t(`details.tabs.${key}`),
    Icon: ICONS[key],
  }));
  const [chosen, setChosen] = useState(initialTab);
  const active = sections.some((s) => s.key === chosen) ? chosen : 'about';

  return (
    <SectionedModal
      title={`#${channel.name ?? t('details.fallbackName')}`}
      sections={sections}
      active={active}
      onSelect={setChosen}
      onClose={onClose}
      qaPrefix="channel-details"
    >
      {active === 'about' && <AboutSection channel={channel} canModerate={canModerate} onClose={onClose} />}
      {active === 'settings' && <SettingsSection channel={channel} onClose={onClose} />}
      {active === 'activity' && <ActivitySection channel={channel} />}
      {active === 'integrations' && (
        <IntegrationsSection
          channel={channel}
          onOpenWorkflow={
            onOpenWorkflow &&
            ((workflowId) => {
              onClose();
              onOpenWorkflow(workflowId);
            })
          }
          onOpenApp={
            onOpenApp &&
            ((app) => {
              onClose();
              onOpenApp(app);
            })
          }
        />
      )}
    </SectionedModal>
  );
}
