import { useId, type ReactNode } from 'react';
import { Check } from 'lucide-react';

export interface CheckboxProps {
  label: string;
  description?: ReactNode;
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
  dataQa?: string;
  className?: string;
}

export function Checkbox({
  label,
  description,
  checked,
  onChange,
  disabled = false,
  dataQa,
  className = '',
}: CheckboxProps) {
  const id = useId();
  return (
    <label
      htmlFor={id}
      data-qa={dataQa}
      className={`flex items-start gap-3 ${disabled ? 'cursor-not-allowed opacity-60' : 'cursor-pointer'} ${className}`}
    >
      <span className="relative mt-0.5 flex h-4 w-4 shrink-0">
        <input
          id={id}
          type="checkbox"
          checked={checked}
          disabled={disabled}
          aria-describedby={description ? `${id}-description` : undefined}
          onChange={(e) => onChange(e.target.checked)}
          className="peer h-4 w-4 cursor-[inherit] appearance-none rounded border border-line-strong bg-raised/50 transition checked:border-purple-600 checked:bg-purple-600 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-purple-500 focus-visible:ring-offset-2 focus-visible:ring-offset-surface"
        />
        <Check
          className="pointer-events-none absolute inset-0 m-auto hidden h-3 w-3 text-white peer-checked:block"
          strokeWidth={3}
        />
      </span>
      <span className="min-w-0">
        <span className="block text-sm font-medium text-fg-dim">{label}</span>
        {description && (
          <span id={`${id}-description`} className="block text-xs text-muted">
            {description}
          </span>
        )}
      </span>
    </label>
  );
}
