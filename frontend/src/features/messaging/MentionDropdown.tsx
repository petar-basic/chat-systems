import { forwardRef, useImperativeHandle, useState } from 'react';
import { AtSign, Hash } from 'lucide-react';

export interface MentionItem {
  id: string;
  label: string;
  type: 'user' | 'channel';
}

interface Props {
  items: MentionItem[];
  command: (item: MentionItem) => void;
}

export interface MentionDropdownHandle {
  onKeyDown: (props: { event: KeyboardEvent }) => boolean;
}

export const MentionDropdown = forwardRef<MentionDropdownHandle, Props>(({ items, command }, ref) => {
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

  if (!items.length) {
    return (
      <div className="bg-slate-800 border border-slate-700 rounded-lg shadow-xl px-3 py-2 text-xs text-slate-400 w-52">
        No results
      </div>
    );
  }

  return (
    <div className="bg-slate-800 border border-slate-700 rounded-lg shadow-xl overflow-hidden w-52">
      {items.map((item, i) => (
        <button
          key={`${item.type}-${item.id}`}
          type="button"
          onMouseDown={(e) => {
            e.preventDefault();
            command(item);
          }}
          className={`w-full flex items-center gap-2 px-3 py-2 text-sm text-left transition cursor-pointer ${
            i === selectedIndex ? 'bg-purple-600/20 text-white' : 'text-slate-300 hover:bg-slate-700/50'
          }`}
        >
          {item.type === 'channel' ? (
            <Hash className="w-3.5 h-3.5 text-slate-400 shrink-0" />
          ) : (
            <AtSign className="w-3.5 h-3.5 text-slate-400 shrink-0" />
          )}
          <span className="truncate">{item.label}</span>
        </button>
      ))}
    </div>
  );
});

MentionDropdown.displayName = 'MentionDropdown';
