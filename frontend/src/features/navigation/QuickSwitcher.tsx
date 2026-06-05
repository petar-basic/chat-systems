import { useEffect, useMemo, useRef, useState } from 'react';
import { Hash, Lock, MessageSquare, Search } from 'lucide-react';
import type { Channel, WorkspaceMember } from '@/stores/workspace';
import { displayNameOf } from '@/lib/userHelpers';
import { useEscapeToClose } from '@/shared/hooks/useEscapeToClose';

interface QuickSwitcherProps {
  channels: Channel[];
  members: WorkspaceMember[];
  currentUserId?: string;
  onSelectChannel: (channel: Channel) => void;
  onSelectDm: (userId: string) => void;
  onClose: () => void;
}

interface Item {
  key: string;
  label: string;
  sub?: string;
  icon: React.ReactNode;
  run: () => void;
}

export function QuickSwitcher({
  channels,
  members,
  currentUserId,
  onSelectChannel,
  onSelectDm,
  onClose,
}: QuickSwitcherProps) {
  const [query, setQuery] = useState('');
  const [active, setActive] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  useEscapeToClose(onClose);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const items = useMemo<Item[]>(() => {
    const q = query.trim().toLowerCase();
    const channelItems: Item[] = channels
      .filter((c) => (c.name || '').toLowerCase().includes(q))
      .map((c) => ({
        key: `ch-${c.id}`,
        label: c.name || 'channel',
        sub: 'Channel',
        icon:
          c.channel_type === 'private' ? (
            <Lock className="w-4 h-4 text-slate-400" />
          ) : (
            <Hash className="w-4 h-4 text-slate-400" />
          ),
        run: () => onSelectChannel(c),
      }));
    const peopleItems: Item[] = members
      .filter((m) => m.user_id !== currentUserId)
      .filter((m) => (m.display_name || m.email).toLowerCase().includes(q))
      .map((m) => ({
        key: `dm-${m.user_id}`,
        label: displayNameOf(m.display_name),
        sub: 'Direct message',
        icon: <MessageSquare className="w-4 h-4 text-purple-400" />,
        run: () => onSelectDm(m.user_id),
      }));
    return [...channelItems, ...peopleItems].slice(0, 50);
  }, [query, channels, members, currentUserId, onSelectChannel, onSelectDm]);

  const activeIndex = items.length ? Math.min(active, items.length - 1) : 0;

  const choose = (item: Item | undefined) => {
    if (!item) return;
    item.run();
    onClose();
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setActive(items.length ? (activeIndex + 1) % items.length : 0);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setActive(items.length ? (activeIndex - 1 + items.length) % items.length : 0);
    } else if (e.key === 'Enter') {
      e.preventDefault();
      choose(items[activeIndex]);
    }
  };

  return (
    <div
      className="fixed inset-0 bg-black/50 flex items-start justify-center pt-[15vh] z-50"
      onClick={onClose}
    >
      <div
        className="bg-slate-800 border border-slate-700 rounded-2xl w-full max-w-lg shadow-2xl overflow-hidden"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-label="Quick switcher"
        data-qa="quick-switcher"
      >
        <div className="flex items-center gap-2 px-4 py-3 border-b border-slate-700">
          <Search className="w-4 h-4 text-slate-400 shrink-0" />
          <input
            ref={inputRef}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={onKeyDown}
            placeholder="Jump to a channel or person…"
            aria-label="Search channels and people"
            className="flex-1 bg-transparent text-white placeholder-slate-500 focus:outline-none text-sm"
          />
        </div>
        <div className="max-h-80 overflow-y-auto py-1">
          {items.length === 0 ? (
            <div className="px-4 py-6 text-center text-sm text-slate-400">No matches</div>
          ) : (
            items.map((item, i) => (
              <button
                key={item.key}
                onMouseEnter={() => setActive(i)}
                onClick={() => choose(item)}
                className={`w-full px-4 py-2 flex items-center gap-3 text-left transition ${
                  i === active ? 'bg-purple-600/20' : 'hover:bg-slate-700/40'
                }`}
              >
                <span className="shrink-0">{item.icon}</span>
                <span className="flex-1 text-sm text-slate-200 truncate">{item.label}</span>
                {item.sub && <span className="text-xs text-slate-400 shrink-0">{item.sub}</span>}
              </button>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
