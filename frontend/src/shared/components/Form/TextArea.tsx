import type { ComponentPropsWithRef } from 'react';
import { Field, type FieldProps } from './Field';
import { fieldStyles } from './fieldStyles';

type AreaProps = Omit<
  ComponentPropsWithRef<'textarea'>,
  'id' | 'className' | 'aria-describedby' | 'aria-invalid'
>;

export interface TextAreaProps extends FieldProps, AreaProps {
  counter?: boolean;
  dataQa?: string;
}

export function TextArea({
  label,
  labelHidden,
  optional,
  hint,
  error,
  className,
  counter = false,
  dataQa,
  rows = 3,
  ...area
}: TextAreaProps) {
  const count =
    counter && area.maxLength !== undefined
      ? { value: String(area.value ?? '').length, max: area.maxLength }
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
        <textarea
          {...area}
          {...control}
          rows={rows}
          data-qa={dataQa}
          className={`block resize-none py-2 leading-5 ${fieldStyles({ invalid: !!error })}`}
        />
      )}
    </Field>
  );
}
