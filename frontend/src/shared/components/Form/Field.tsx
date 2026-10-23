import { useId, type ReactNode } from 'react';

export interface FieldProps {
  label: string;
  labelHidden?: boolean;
  optional?: boolean;
  hint?: ReactNode;
  error?: string | null;
  count?: { value: number; max: number };
  className?: string;
}

export interface FieldControl {
  id: string;
  'aria-describedby': string | undefined;
  'aria-invalid': true | undefined;
}

export function Field({
  label,
  labelHidden = false,
  optional = false,
  hint,
  error,
  count,
  className = '',
  children,
}: FieldProps & { children: (control: FieldControl) => ReactNode }) {
  const id = useId();
  const hintId = `${id}-hint`;
  const errorId = `${id}-error`;
  const described = error ? errorId : hint ? hintId : undefined;
  const over = count ? count.value > count.max : false;

  return (
    <div className={className}>
      <div className={labelHidden ? 'sr-only' : 'mb-1.5 flex items-baseline justify-between gap-3'}>
        <label htmlFor={id} className="text-sm font-medium text-fg-dim">
          {label}
          {optional && <span className="font-normal text-muted"> (optional)</span>}
        </label>
        {count && !labelHidden && (
          <span className={`text-xs tabular-nums ${over ? 'text-danger' : 'text-muted'}`} aria-hidden>
            {count.max - count.value}
          </span>
        )}
      </div>
      {children({ id, 'aria-describedby': described, 'aria-invalid': error ? true : undefined })}
      {error && (
        <p id={errorId} role="alert" className="mt-1.5 text-xs text-danger">
          {error}
        </p>
      )}
      {hint && !error && (
        <p id={hintId} className="mt-1.5 text-xs text-muted">
          {hint}
        </p>
      )}
    </div>
  );
}
