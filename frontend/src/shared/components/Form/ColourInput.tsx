import { useState } from 'react';
import { Field, type FieldProps } from './Field';
import { fieldStyles } from './fieldStyles';

export interface ColourInputProps extends Omit<FieldProps, 'count' | 'error'> {
  value: string;
  onChange: (value: string) => void;
  dataQa?: string;
}

const HEX = /^#[0-9a-f]{6}$/i;

export function ColourInput({
  label,
  labelHidden,
  optional,
  hint,
  className,
  value,
  onChange,
  dataQa,
}: ColourInputProps) {
  const [draft, setDraft] = useState<string | null>(null);
  const [seen, setSeen] = useState(value);
  if (seen !== value) {
    setSeen(value);
    setDraft(null);
  }
  const shown = draft ?? value;
  const invalid = draft !== null && !HEX.test(draft);

  return (
    <Field
      label={label}
      labelHidden={labelHidden}
      optional={optional}
      hint={hint}
      className={className}
      error={invalid ? 'Use a hex colour like #1164a3.' : null}
    >
      {(control) => (
        <div className="relative">
          <input
            type="color"
            tabIndex={-1}
            aria-hidden="true"
            value={value.toLowerCase()}
            onChange={(event) => onChange(event.target.value.toLowerCase())}
            className="absolute inset-y-0 left-1.5 my-auto h-5 w-5 cursor-pointer rounded border border-line bg-transparent p-0 [&::-webkit-color-swatch]:rounded-sm [&::-webkit-color-swatch]:border-none [&::-webkit-color-swatch-wrapper]:p-0"
          />
          <input
            {...control}
            type="text"
            value={shown}
            maxLength={7}
            spellCheck={false}
            autoComplete="off"
            data-qa={dataQa}
            onChange={(event) => {
              const next = event.target.value.trim();
              setDraft(next);
              if (HEX.test(next)) onChange(next.toLowerCase());
            }}
            onBlur={() => {
              if (!invalid) setDraft(null);
            }}
            className={`h-9 font-mono ${fieldStyles({ invalid, leading: true })}`}
          />
        </div>
      )}
    </Field>
  );
}
