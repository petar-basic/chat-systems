import type { ComponentPropsWithRef, ReactNode } from 'react';
import { Field, type FieldProps } from './Field';
import { fieldStyles } from './fieldStyles';

type InputProps = Omit<
  ComponentPropsWithRef<'input'>,
  'id' | 'className' | 'aria-describedby' | 'aria-invalid'
>;

export interface TextInputProps extends FieldProps, InputProps {
  leading?: ReactNode;
  counter?: boolean;
  dataQa?: string;
}

export function TextInput({
  label,
  labelHidden,
  optional,
  hint,
  error,
  className,
  leading,
  counter = false,
  dataQa,
  type = 'text',
  ...input
}: TextInputProps) {
  const count =
    counter && input.maxLength !== undefined
      ? { value: String(input.value ?? '').length, max: input.maxLength }
      : undefined;
  return (
    <Field
      label={label}
      labelHidden={labelHidden}
      optional={optional}
      hint={hint}
      error={error}
      count={count}
      className={className}
    >
      {(control) => (
        <div className="relative">
          {leading && (
            <span className="pointer-events-none absolute inset-y-0 left-3 flex items-center text-muted">
              {leading}
            </span>
          )}
          <input
            {...input}
            {...control}
            type={type}
            data-qa={dataQa}
            className={`h-9 ${fieldStyles({ invalid: !!error, leading: !!leading })}`}
          />
        </div>
      )}
    </Field>
  );
}
