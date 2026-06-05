import { usePresenceStore, type PresenceStatus } from '../stores/presence';

interface Props {
  userId: string;
  className?: string;
}

const STATUS: Record<PresenceStatus, { cls: string; label: string }> = {
  online: { cls: 'bg-green-500', label: 'Online' },
  away: { cls: 'bg-amber-500 ring-2 ring-amber-500/30', label: 'Away' },
  offline: { cls: 'bg-transparent border border-slate-500', label: 'Offline' },
};

export default function PresenceDot({ userId, className = '' }: Props) {
  const status = usePresenceStore((s) => s.getStatus(userId));
  const { cls, label } = STATUS[status];

  return (
    <span
      role="img"
      aria-label={`Status: ${label}`}
      title={label}
      className={`inline-block w-2.5 h-2.5 rounded-full ${cls} ${className}`}
    />
  );
}
