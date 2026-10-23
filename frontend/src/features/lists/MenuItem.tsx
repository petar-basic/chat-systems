import type { ReactNode } from 'react';

export function MenuItem({
  icon,
  label,
  onSelect,
  danger = false,
  disabled = false,
  first = false,
  qa,
}: {
  icon: ReactNode;
  label: string;
  onSelect: () => void;
  danger?: boolean;
  disabled?: boolean;
  first?: boolean;
  qa: string;
}) {
  return (
    <button
      role="menuitem"
      data-autofocus={first || undefined}
      disabled={disabled}
      onClick={onSelect}
      data-qa={qa}
      className={`w-full px-3 py-2 text-left text-sm flex items-center gap-2 rounded cursor-pointer disabled:cursor-not-allowed disabled:opacity-50 focus:outline-none focus:bg-raised hover:bg-raised ${
        danger ? 'text-danger' : 'text-fg-dim'
      }`}
    >
      {icon}
      {label}
    </button>
  );
}
