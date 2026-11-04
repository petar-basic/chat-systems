import { Clock3 } from 'lucide-react';
import { useDisplayStore } from '@/stores/display';
import { localTime } from '@/lib/datetime';

export function LocalTimeLine({ timezone }: { timezone?: string | null }) {
  const clock = useDisplayStore((s) => s.clock);
  const time = localTime(timezone, new Date(), clock);
  if (!time) return null;
  return (
    <div className="flex items-center gap-1.5 text-xs text-muted" data-qa="hover-card-local-time">
      <Clock3 className="w-3 h-3 shrink-0" />
      {time} local time
    </div>
  );
}
