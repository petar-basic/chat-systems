import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Send } from 'lucide-react';
import type { Channel } from '@/stores/workspace';
import { ScheduledList, type Conversation } from '@/features/messaging';
import { PanelHeader } from '@/shared/components/PanelHeader/PanelHeader';
import { DraftsList } from './DraftsList';
import { SentList } from './SentList';

type Tab = 'drafts' | 'scheduled' | 'sent';

interface Props {
  workspaceId: string;
  instanceUrl?: string;
  channels: Channel[];
  conversations: Conversation[];
  onClose: () => void;
  onOpenMessage: (messageId: string, thread?: boolean) => void;
}

export function DraftsPanel({
  workspaceId,
  instanceUrl,
  channels,
  conversations,
  onClose,
  onOpenMessage,
}: Props) {
  const { t } = useTranslation('messages');
  const [tab, setTab] = useState<Tab>('drafts');
  const tabs: { id: Tab; label: string }[] = [
    { id: 'drafts', label: t('drafts.tab') },
    { id: 'scheduled', label: t('scheduled.title') },
    { id: 'sent', label: t('sent.tab') },
  ];

  return (
    <div
      className="w-full lg:w-80 max-lg:fixed max-lg:inset-0 max-lg:z-40 bg-surface border-l border-line/50 flex flex-col"
      data-qa="drafts-panel"
    >
      <PanelHeader
        title={t('drafts.title')}
        icon={<Send className="h-4 w-4 text-muted" />}
        onClose={onClose}
      />
      <div className="flex gap-1 border-b border-line/50 px-3 py-2" role="tablist">
        {tabs.map((item) => (
          <button
            key={item.id}
            type="button"
            role="tab"
            aria-selected={tab === item.id}
            data-qa={`drafts-tab-${item.id}`}
            onClick={() => setTab(item.id)}
            className={`rounded-full px-2.5 py-1 text-xs transition ${
              tab === item.id ? 'bg-purple-600 text-white' : 'text-fg-dim hover:bg-raised'
            }`}
          >
            {item.label}
          </button>
        ))}
      </div>
      {tab === 'drafts' && (
        <DraftsList
          workspaceId={workspaceId}
          channels={channels}
          conversations={conversations}
          onOpenThread={(rootId) => onOpenMessage(rootId, true)}
          onOpened={onClose}
        />
      )}
      {tab === 'scheduled' && (
        <ScheduledList
          workspaceId={workspaceId}
          instanceUrl={instanceUrl}
          channels={channels}
          conversations={conversations}
        />
      )}
      {tab === 'sent' && (
        <SentList
          workspaceId={workspaceId}
          instanceUrl={instanceUrl}
          channels={channels}
          conversations={conversations}
          onOpenMessage={onOpenMessage}
        />
      )}
    </div>
  );
}
