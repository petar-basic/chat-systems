import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { LayoutGrid, Maximize2, Expand, Shrink } from 'lucide-react';
import { useHuddleStore } from '@/stores/huddle';
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
  const localHandRaised = useHuddleStore((s) => s.localHandRaised);
  const localVideoStream = useHuddleStore((s) => s.localVideoStream);
  const localScreenStream = useHuddleStore((s) => s.localScreenStream);
  const localStream = useHuddleStore((s) => s.localStream);
  const [layout, setLayout] = useState<Layout>('grid');
  const [expanded, setExpanded] = useState(false);

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
          : 'fixed bottom-4 right-4 z-60 w-120 max-w-[calc(100vw-2rem)] bg-surface border border-line rounded-2xl shadow-2xl p-3 flex flex-col gap-3'
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

      <HuddleStage tiles={tiles} self={self} layout={layout} expanded={expanded} />

      <HuddleControlBar controls={controls} active={active} />

      {!localStream ? (
        <p className="text-xs text-muted text-center">Connecting…</p>
      ) : (
        remotes.length === 0 && <p className="text-xs text-muted text-center">Waiting for others to join…</p>
      )}
    </div>
  );
}
