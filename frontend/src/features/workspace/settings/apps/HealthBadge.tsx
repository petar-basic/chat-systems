import { useTranslation } from 'react-i18next';
import type { AppHealth } from '@/features/workspace/hooks/useAppDirectory';

const TONES: Record<Exclude<AppHealth['state'], 'ok'>, string> = {
  off: 'bg-raised text-muted',
  disabled: 'bg-warning/10 text-warning',
  failing: 'bg-danger/10 text-danger',
};

export function HealthBadge({ health }: { health: AppHealth }) {
  const { t } = useTranslation('workspace');
  if (health.state === 'ok') return null;
  const tone = TONES[health.state];
  return (
    <span
      className={`shrink-0 rounded px-1 py-0.5 text-3xs font-semibold uppercase tracking-wide ${tone}`}
      data-qa="app-health"
    >
      {t(`settings.apps.health.${health.state}`)}
    </span>
  );
}
