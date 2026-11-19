import { useRef } from 'react';
import { insertAt } from './workflowModel';

export function useVariableInsert<T extends HTMLInputElement | HTMLTextAreaElement>(
  value: string,
  onValue: (value: string) => void,
) {
  const ref = useRef<T>(null);
  const insert = (token: string) => {
    const field = ref.current;
    const start = field?.selectionStart ?? value.length;
    const end = field?.selectionEnd ?? start;
    const { text, caret } = insertAt(value, token, start, end);
    onValue(text);
    requestAnimationFrame(() => {
      field?.focus();
      field?.setSelectionRange(caret, caret);
    });
  };
  return { ref, insert };
}
