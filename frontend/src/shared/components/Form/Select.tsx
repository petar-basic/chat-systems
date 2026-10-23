import type { ComponentPropsWithRef } from 'react';
import { ChevronDown } from 'lucide-react';
import { Field, type FieldProps } from './Field';
import { fieldStyles } from './fieldStyles';

type NativeSelectProps = Omit<
  ComponentPropsWithRef<'select'>,
  'id' | 'className' | 'aria-describedby' | 'aria-invalid'
>;

export interface SelectProps extends FieldProps, NativeSelectProps {
  dataQa?: string;
}

export function Select({
  label,
  labelHidden,
  optional,
  hint,
  error,
  className,
  dataQa,
  children,
  ...select
}: SelectProps) {
  return (
    <Field
      label={label}
      labelHidden={labelHidden}
      optional={optional}
      hint={hint}
      error={error}
      className={className}
    >
      {(control) => (
        <div className="relative">
          <select
            {...select}
            {...control}
            data-qa={dataQa}
            className={`h-9 cursor-pointer appearance-none ${fieldStyles({ invalid: !!error, trailing: true })}`}
          >
            {children}
          </select>
          <ChevronDown className="pointer-events-none absolute top-1/2 right-2.5 h-4 w-4 -translate-y-1/2 text-muted" />
        </div>
      )}
    </Field>
  );
}
