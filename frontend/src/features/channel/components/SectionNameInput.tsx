import { useRef, useState } from 'react';

interface Props {
  initial?: string;
  label: string;
  onSubmit: (name: string) => void;
  onCancel: () => void;
}

export function SectionNameInput({ initial = '', label, onSubmit, onCancel }: Props) {
  const [value, setValue] = useState(initial);
  const settled = useRef(false);
  const cancel = () => {
    if (settled.current) return;
    settled.current = true;
    onCancel();
  };
  const submit = () => {
    if (settled.current) return;
    settled.current = true;
    const name = value.trim();
    if (name) onSubmit(name);
    else onCancel();
  };
  return (
    <input
      autoFocus
      aria-label={label}
      value={value}
      maxLength={80}
      onChange={(e) => setValue(e.target.value)}
      onBlur={submit}
      onKeyDown={(e) => {
        if (e.key === 'Enter') {
          e.preventDefault();
          submit();
        }
        if (e.key === 'Escape') {
          e.preventDefault();
          cancel();
        }
      }}
      data-qa="sidebar-section-name-input"
      className="w-full px-2 py-1 text-sm bg-raised/50 border border-line-strong rounded text-fg focus:outline-none focus:ring-2 focus:ring-purple-500"
    />
  );
}
