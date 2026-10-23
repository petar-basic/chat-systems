import { useRef, useState } from 'react';
import { TextInput } from '@/shared/components/Form/TextInput';

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
    <TextInput
      label={label}
      labelHidden
      compact
      autoFocus
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
      dataQa="sidebar-section-name-input"
    />
  );
}
