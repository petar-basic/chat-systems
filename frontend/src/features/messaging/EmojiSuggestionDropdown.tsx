import { forwardRef, useImperativeHandle, useState } from 'react';
import type { EmojiSuggestionItem } from '@/lib/emojiData';

interface Props {
  items: EmojiSuggestionItem[];
  command: (item: EmojiSuggestionItem) => void;
}

export interface EmojiSuggestionHandle {
  onKeyDown: (props: { event: KeyboardEvent }) => boolean;
}

export const EmojiSuggestionDropdown = forwardRef<EmojiSuggestionHandle, Props>(({ items, command }, ref) => {
  const [selectedIndex, setSelectedIndex] = useState(0);

  const [prevItems, setPrevItems] = useState(items);
  if (items !== prevItems) {
    setPrevItems(items);
    setSelectedIndex(0);
  }

  useImperativeHandle(ref, () => ({
    onKeyDown: ({ event }) => {
      if (items.length === 0) return false;
      if (event.key === 'ArrowUp') {
        setSelectedIndex((i) => (i + items.length - 1) % items.length);
        return true;
      }
      if (event.key === 'ArrowDown') {
        setSelectedIndex((i) => (i + 1) % items.length);
        return true;
      }
      if (event.key === 'Enter' || event.key === 'Tab') {
        const item = items[selectedIndex];
        if (item) command(item);
        return true;
      }
      return false;
    },
  }));

  if (!items.length) return null;

  return (
    <div className="bg-surface border border-line rounded-lg shadow-xl overflow-hidden w-60 max-h-60 overflow-y-auto">
      {items.map((item, i) => (
        <button
          key={item.id}
          type="button"
          onMouseDown={(e) => {
            e.preventDefault();
            command(item);
          }}
          className={`flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm transition cursor-pointer ${
            i === selectedIndex ? 'bg-purple-600/20 text-fg' : 'text-fg-dim hover:bg-raised/50'
          }`}
        >
          <span className="text-lg leading-none">{item.native}</span>
          <span className="truncate text-muted">:{item.id}:</span>
        </button>
      ))}
    </div>
  );
});

EmojiSuggestionDropdown.displayName = 'EmojiSuggestionDropdown';
