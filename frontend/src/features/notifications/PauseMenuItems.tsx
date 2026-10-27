import { Moon } from 'lucide-react';
import { useDndStatus, useSetDnd } from '@/features/notifications/hooks/useNotifications';
import { useWorkspaceStore } from '@/stores/workspace';
import { PAUSE_PRESETS, currentPause, minutesFromNowIso, pauseLabel, tomorrowMorningIso } from '@/lib/pause';

const ITEM = 'w-full px-3 py-1.5 text-left text-sm text-fg-dim hover:bg-raised cursor-pointer';

export function PauseMenuItems({ onDone }: { onDone: () => void }) {
  const { data: status } = useDndStatus();
  const setDnd = useSetDnd();
  const pause = currentPause(status);
  const workspace = useWorkspaceStore((s) => s.currentWorkspace?.name);
  const where = workspace ? ` in ${workspace}` : '';

  const choose = (until: string | null) => {
    setDnd.mutate(until);
    onDone();
  };

  if (pause) {
    return (
      <div data-qa="pause-menu">
        <div className="px-3 py-1.5 flex items-center gap-2 text-xs text-muted" data-qa="pause-status">
          <Moon className="w-3.5 h-3.5 shrink-0 text-accent" />
          {pauseLabel(pause)}
        </div>
        <button
          type="button"
          className={`${ITEM} text-accent-soft`}
          onClick={() => choose(null)}
          data-qa="pause-resume"
        >
          Resume notifications{where}
        </button>
      </div>
    );
  }

  return (
    <div data-qa="pause-menu">
      <div className="px-3 py-1 text-[10px] uppercase tracking-wide text-subtle" data-qa="pause-heading">
        Pause notifications{where}
      </div>
      {PAUSE_PRESETS.map((preset) => (
        <button
          key={preset.minutes}
          type="button"
          className={ITEM}
          onClick={() => choose(minutesFromNowIso(preset.minutes))}
        >
          {preset.label}
        </button>
      ))}
      <button type="button" className={ITEM} onClick={() => choose(tomorrowMorningIso())}>
        Until tomorrow
      </button>
    </div>
  );
}
