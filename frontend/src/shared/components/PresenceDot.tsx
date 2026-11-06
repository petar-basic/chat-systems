import { useTranslation } from 'react-i18next';
import { usePresenceStore, type PresenceStatus } from '@/stores/presence';

interface Props {
  userId: string;
  className?: string;
}

const STATUS: Record<PresenceStatus, string> = {
  online: 'bg-presence',
  away: 'bg-amber-500 ring-2 ring-amber-500/30',
  offline: 'bg-transparent border border-line-strong',
};

export default function PresenceDot({ userId, className = '' }: Props) {
  const { t } = useTranslation('common');
  const status = usePresenceStore((s) => s.getStatus(userId));
  const cls = STATUS[status];
  const label = t(`presence.${status}`);

  return (
    <span
      role="img"
      aria-label={t('presence.status', { status: label })}
      data-tooltip={label}
      className={`inline-block w-2.5 h-2.5 rounded-full ${cls} ${className}`}
    />
  );
}
