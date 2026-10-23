import type { ReactNode } from 'react';

interface Props {
  label: string;
  onFiles: (files: File[]) => void;
  className: string;
  children: ReactNode;
  accept?: string;
  multiple?: boolean;
  disabled?: boolean;
  title?: string;
  dataQa?: string;
}

export function FilePicker({
  label,
  onFiles,
  className,
  children,
  accept,
  multiple,
  disabled,
  title,
  dataQa,
}: Props) {
  return (
    <label
      title={title}
      data-qa={dataQa}
      aria-disabled={disabled || undefined}
      className={`relative has-[:focus-visible]:ring-2 has-[:focus-visible]:ring-purple-500 ${
        disabled ? 'pointer-events-none opacity-50' : 'cursor-pointer'
      } ${className}`}
    >
      <input
        type="file"
        aria-label={label}
        accept={accept}
        multiple={multiple}
        disabled={disabled}
        className="sr-only"
        onChange={(e) => {
          const files = Array.from(e.target.files ?? []);
          e.target.value = '';
          if (files.length > 0) onFiles(files);
        }}
      />
      {children}
    </label>
  );
}
