import { useId, type ReactNode } from 'react';

export interface RadioCardOption<T extends string> {
  value: T;
  label: string;
  description?: ReactNode;
  icon?: ReactNode;
  dataQa?: string;
}

export interface RadioCardsProps<T extends string> {
  label: string;
  value: T;
  options: RadioCardOption<T>[];
  onChange: (value: T) => void;
  labelHidden?: boolean;
  columns?: 1 | 2;
  className?: string;
}

export function RadioCards<T extends string>({
  label,
  value,
  options,
  onChange,
  labelHidden = false,
  columns = 2,
  className = '',
}: RadioCardsProps<T>) {
  const name = useId();
  return (
    <fieldset className={className}>
      <legend className={labelHidden ? 'sr-only' : 'mb-1.5 text-sm font-medium text-fg-dim'}>{label}</legend>
      <div className={`grid gap-2 ${columns === 2 ? 'sm:grid-cols-2' : ''}`}>
        {options.map((option) => {
          const checked = option.value === value;
          return (
            <label
              key={option.value}
              data-qa={option.dataQa}
              className={`flex cursor-pointer items-start gap-3 rounded-lg border p-3 transition has-[:focus-visible]:ring-2 has-[:focus-visible]:ring-purple-500 ${
                checked ? 'border-purple-500 bg-purple-600/10' : 'border-line-strong hover:bg-raised/40'
              }`}
            >
              <input
                type="radio"
                name={name}
                value={option.value}
                checked={checked}
                onChange={() => onChange(option.value)}
                className="sr-only"
              />
              {option.icon && (
                <span className={`mt-0.5 shrink-0 ${checked ? 'text-accent-soft' : 'text-muted'}`}>
                  {option.icon}
                </span>
              )}
              <span className="min-w-0">
                <span className="block text-sm font-medium text-fg">{option.label}</span>
                {option.description && (
                  <span className="mt-0.5 block text-xs text-muted">{option.description}</span>
                )}
              </span>
            </label>
          );
        })}
      </div>
    </fieldset>
  );
}
