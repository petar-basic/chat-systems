import { forwardRef, useImperativeHandle, useState } from 'react';
import type { SlashCommand } from './hooks/useSlashCommands';

interface Props {
  items: SlashCommand[];
  command: (item: SlashCommand) => void;
}

export interface CommandSuggestionHandle {
  onKeyDown: (props: { event: KeyboardEvent }) => boolean;
}

export const CommandSuggestionDropdown = forwardRef<CommandSuggestionHandle, Props>(
  ({ items, command }, ref) => {
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
      <div
        role="listbox"
        aria-label="Commands"
        data-qa="command-suggestions"
        className="bg-surface border border-line rounded-lg shadow-xl overflow-hidden w-80 max-h-72 overflow-y-auto"
      >
        {items.map((item, i) => (
          <button
            key={item.command}
            type="button"
            role="option"
            aria-selected={i === selectedIndex}
            data-qa="command-suggestion"
            onMouseDown={(e) => {
              e.preventDefault();
              command(item);
            }}
            className={`flex w-full flex-col px-3 py-1.5 text-left transition cursor-pointer ${
              i === selectedIndex ? 'bg-purple-600/20' : 'hover:bg-raised/50'
            }`}
          >
            <span className="text-sm font-medium text-fg">/{item.command}</span>
            {item.hint && <span className="text-xs text-muted truncate">{item.hint}</span>}
          </button>
        ))}
      </div>
    );
  },
);

CommandSuggestionDropdown.displayName = 'CommandSuggestionDropdown';
