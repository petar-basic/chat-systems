import { useTranslation } from 'react-i18next';
import { formatResume, type Pause } from '@/lib/pause';

export function usePauseLabel() {
  const { t } = useTranslation('notifications');
  return (pause: Pause, now = new Date()) =>
    t(pause.bySchedule ? 'pause.bySchedule' : 'pause.until', { time: formatResume(pause.until, now) });
}
