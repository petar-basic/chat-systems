import { useEffect, useState } from 'react';
import { Moon } from 'lucide-react';
import { useUserCache } from '@/stores/users';
import { useDisplayStore } from '@/stores/display';
import { displayNameOf } from '@/lib/userHelpers';
import { localTime, offHours } from '@/lib/datetime';

const MINUTE_MS = 60_000;

export function OffHoursNote({ userId }: { userId: string }) {
  const person = useUserCache((s) => s.getUser(userId));
  const clock = useDisplayStore((s) => s.clock);
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const timer = setInterval(() => setNow(new Date()), MINUTE_MS);
    return () => clearInterval(timer);
  }, []);

  const when = offHours(person?.timezone, now);
  const time = localTime(person?.timezone, now, clock);
  if (!person || !when || !time) return null;

  return (
    <p className="flex items-center gap-1.5 px-4 pb-1 text-xs text-muted" data-qa="off-hours-note">
      <Moon className="w-3 h-3 shrink-0" />
      It is {time} for {displayNameOf(person.display_name)}, {when === 'late' ? 'late' : 'early'} in their
      day.
    </p>
  );
}
