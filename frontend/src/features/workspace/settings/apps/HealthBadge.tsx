import type { AppHealth } from '@/features/workspace/hooks/useAppDirectory';

const LOOKS: Record<AppHealth['state'], { label: string; tone: string } | null> = {
  ok: null,
  off: { label: 'Off', tone: 'bg-raised text-muted' },
  disabled: { label: 'Turned off', tone: 'bg-warning/10 text-warning' },
  failing: { label: 'Failing', tone: 'bg-danger/10 text-danger' },
};

export function HealthBadge({ health }: { health: AppHealth }) {
  const look = LOOKS[health.state];
  if (!look) return null;
  return (
    <span
      className={`shrink-0 rounded px-1 py-0.5 text-[10px] font-semibold uppercase tracking-wide ${look.tone}`}
      data-qa="app-health"
    >
      {look.label}
    </span>
  );
}
