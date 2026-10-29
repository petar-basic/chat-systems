import { useState } from 'react';
import { Info, Plug, SlidersHorizontal } from 'lucide-react';
import { SectionedModal, type ModalSection } from '@/shared/components/Modal/SectionedModal';
import type { Channel } from '@/stores/workspace';
import type { AppRef } from '@/features/workspace';
import { AboutSection } from './AboutSection';
import { SettingsSection } from './SettingsSection';
import { IntegrationsSection } from './IntegrationsSection';

export type ChannelDetailsTab = 'about' | 'settings' | 'integrations';

const ABOUT: ModalSection<ChannelDetailsTab> = { key: 'about', label: 'About', Icon: Info };
const SETTINGS: ModalSection<ChannelDetailsTab> = {
  key: 'settings',
  label: 'Settings',
  Icon: SlidersHorizontal,
};
const INTEGRATIONS: ModalSection<ChannelDetailsTab> = {
  key: 'integrations',
  label: 'Integrations',
  Icon: Plug,
};

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
  const sections = canModerate ? [ABOUT, SETTINGS, INTEGRATIONS] : [ABOUT, INTEGRATIONS];
  const [chosen, setChosen] = useState(initialTab);
  const active = sections.some((s) => s.key === chosen) ? chosen : 'about';

  return (
    <SectionedModal
      title={`#${channel.name ?? 'channel'}`}
      sections={sections}
      active={active}
      onSelect={setChosen}
      onClose={onClose}
      qaPrefix="channel-details"
    >
      {active === 'about' && <AboutSection channel={channel} canModerate={canModerate} onClose={onClose} />}
      {active === 'settings' && <SettingsSection channel={channel} onClose={onClose} />}
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
