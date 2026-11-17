import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { LayoutGrid, Maximize2, Expand, Shrink } from 'lucide-react';
import { useHuddleStore } from '@/stores/huddle';
import { useWorkspaceStore } from '@/stores/workspace';
import { ThreadPanel, useKnownThreadState } from '@/features/threads';
import { useWorkspaceMembers } from '@/features/workspace';
import { useHuddleThread } from '../hooks/useHuddle';
import type { HuddleControls } from '../HuddleController';
import { buildTiles } from './huddleTiles';
import { HuddleStage, type Layout } from './HuddleStage';
import { HuddleControlBar } from './HuddleControlBar';

export function HuddleWindow({ controls }: { controls: HuddleControls }) {
  const { t } = useTranslation('huddle');
  const active = useHuddleStore((s) => s.active);
  const participants = useHuddleStore((s) => s.participants);
  const localMuted = useHuddleStore((s) => s.localMuted);
  const localCameraOn = useHuddleStore((s) => s.localCameraOn);
  const localSharing = useHuddleStore((s) => s.localSharing);
  const localDrawing = useHuddleStore((s) => s.localDrawing);
  const localHandRaised = useHuddleStore((s) => s.localHandRaised);
  const localVideoStream = useHuddleStore((s) => s.localVideoStream);
  const localScreenStream = useHuddleStore((s) => s.localScreenStream);
  const localStream = useHuddleStore((s) => s.localStream);
  const [layout, setLayout] = useState<Layout>('grid');
  const [expanded, setExpanded] = useState(false);
  const [threadFor, setThreadFor] = useState<string | null>(null);
  const currentWorkspaceId = useWorkspaceStore((s) => s.currentWorkspace?.id);
  const here = !!active && active.workspaceId === currentWorkspaceId;
  const threadOpen = here && threadFor === active.huddleId;
  const { data: huddleThread } = useHuddleThread(
    active?.huddleId ?? '',
    active?.instanceUrl,
    here && !!localStream,
  );
  const { data: threadState } = useKnownThreadState(huddleThread?.message.id);
  const { data: members } = useWorkspaceMembers(threadOpen ? active.workspaceId : null, active?.instanceUrl);

  if (!active) return null;

  const self = active.selfUserId;
  const remotes = Object.values(participants).filter((p) => p.userId !== self);
  const tiles = buildTiles(
    {
      self,
      videoStream: localVideoStream,
      screenStream: localScreenStream,
      muted: localMuted,
      cameraOn: localCameraOn,
      sharing: localSharing,
      drawing: localDrawing,
      handRaised: localHandRaised,
    },
    remotes,
  );

  return (
    <div
      role="dialog"
      aria-label={t('window.title')}
      className={
        expanded
          ? 'fixed inset-0 z-60 bg-app p-4 flex flex-col gap-3'
          : `fixed bottom-4 right-4 z-60 ${threadOpen ? 'w-200' : 'w-120'} max-w-[calc(100vw-2rem)] bg-surface border border-line rounded-2xl shadow-2xl p-3 flex flex-col gap-3`
      }
    >
      <div className="flex items-center justify-between">
        <span className="text-sm font-semibold text-fg">
          {t('window.title')}{' '}
          <span className="text-muted font-normal">{t('window.people', { count: tiles.length })}</span>
        </span>
        <div className="flex items-center gap-1">
          <button
            onClick={() => setLayout((l) => (l === 'grid' ? 'focus' : 'grid'))}
            aria-label={layout === 'grid' ? t('window.focusView') : t('window.gridView')}
            data-tooltip={layout === 'grid' ? t('window.focusView') : t('window.gridView')}
            className="p-1.5 rounded-lg text-muted hover:text-fg hover:bg-raised/50 transition"
          >
            {layout === 'grid' ? <Maximize2 className="w-4 h-4" /> : <LayoutGrid className="w-4 h-4" />}
          </button>
          <button
            onClick={() => setExpanded((v) => !v)}
            aria-label={expanded ? t('window.collapse') : t('window.expand')}
            data-tooltip={expanded ? t('window.collapseTip') : t('window.fullScreenTip')}
            className="p-1.5 rounded-lg text-muted hover:text-fg hover:bg-raised/50 transition"
          >
            {expanded ? <Shrink className="w-4 h-4" /> : <Expand className="w-4 h-4" />}
          </button>
        </div>
      </div>

      <div className={`flex gap-3 min-h-0 ${expanded ? 'flex-1' : ''}`}>
        <div
          className={`flex-1 min-w-0 min-h-0 flex flex-col ${threadOpen && huddleThread ? 'max-sm:hidden' : ''}`}
        >
          <HuddleStage tiles={tiles} self={self} layout={layout} expanded={expanded} />
        </div>
        {threadOpen && huddleThread && (
          <div className={`flex min-h-0 max-sm:flex-1 ${expanded ? '' : 'h-112 max-h-[60vh]'}`}>
            <ThreadPanel
              key={huddleThread.message.id}
              parentMessage={huddleThread.message}
              members={members}
              variant="huddle"
              inChannel={huddleThread.in_channel}
              onClose={() => setThreadFor(null)}
            />
          </div>
        )}
      </div>

      <HuddleControlBar
        controls={controls}
        active={active}
        thread={
          huddleThread
            ? {
                open: threadOpen,
                unread: threadState?.unread_count ?? 0,
                onToggle: () => setThreadFor(threadOpen ? null : active.huddleId),
              }
            : undefined
        }
      />

      {!localStream ? (
        <p className="text-xs text-muted text-center">Connecting…</p>
      ) : (
        remotes.length === 0 && <p className="text-xs text-muted text-center">Waiting for others to join…</p>
      )}
    </div>
  );
}
