import { useCallback, useEffect, useLayoutEffect, useRef, useState, type ReactNode } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { ArrowDown } from 'lucide-react';

const BOTTOM_THRESHOLD_PX = 100;
const OLDER_THRESHOLD_PX = 200;

export interface VirtualRow {
  key: string;
  estimatedHeight: number;
}

interface Props<R extends VirtualRow> {
  rows: R[];
  renderRow: (row: R) => ReactNode;
  hasOlder: boolean;
  isLoadingOlder: boolean;
  onLoadOlder: () => void;
  /** Scrolls the row with this key into view once, for permalinks and jumps. */
  scrollToKey?: string;
  onScrollToKeyHandled?: () => void;
  qa?: string;
  ariaLabel?: string;
}

/**
 * One list for channels and conversations. Both had the same structure and the
 * same three scroll behaviours to get right, and two copies of those would drift.
 *
 * The previous implementation leaned on `flex-col-reverse`, which gives
 * bottom-anchoring for free but cannot be windowed. Anchoring is explicit here
 * instead: stick to the bottom only when already there, hold the viewport still
 * when an older page is prepended, and offer a jump when a message arrives while
 * the reader is somewhere above.
 */
export default function VirtualMessageList<R extends VirtualRow>({
  rows,
  renderRow,
  hasOlder,
  isLoadingOlder,
  onLoadOlder,
  scrollToKey,
  onScrollToKeyHandled,
  qa,
  ariaLabel,
}: Props<R>) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const [atBottom, setAtBottom] = useState(true);
  const [hasUnseen, setHasUnseen] = useState(false);

  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: (index) => rows[index]?.estimatedHeight ?? 44,
    getItemKey: (index) => rows[index]?.key ?? index,
    overscan: 8,
  });

  const isAtBottom = useCallback(() => {
    const el = scrollRef.current;
    if (!el) return true;
    return el.scrollHeight - el.scrollTop - el.clientHeight < BOTTOM_THRESHOLD_PX;
  }, []);

  const jumpToBottom = useCallback((behavior: ScrollBehavior = 'auto') => {
    const el = scrollRef.current;
    if (!el) return;
    el.scrollTo({ top: el.scrollHeight, behavior });
    setHasUnseen(false);
  }, []);

  // How far the reader was from the end of the list. Prepending older messages
  // does not change that distance, so restoring it puts the viewport exactly
  // where it was — without needing to identify a row, which cannot be done
  // reliably here: the scroll handler runs before React re-windows, so the
  // mounted rows still belong to the previous scroll position.
  const anchor = useRef<number | null>(null);
  // The distance is only restored once the rows have actually grown at the
  // front. The layout effect runs before that too — on the pass that merely
  // flips the loading flag — and restoring then is a no-op that would throw the
  // anchor away before it was ever needed.
  const anchorArmed = useRef(false);
  // Set when the list should be pinned to the bottom and cleared once it
  // actually is. One shot is not enough: the rows mount at an estimated height
  // and are re-measured after, so the first scroll lands short of the real
  // bottom and the newest message stays outside the window.
  const stick = useRef(false);
  const firstKey = useRef<string | undefined>(rows[0]?.key);
  const lastKey = useRef<string | undefined>(rows[rows.length - 1]?.key);
  const settled = useRef(false);

  const handleScroll = useCallback(() => {
    const el = scrollRef.current;
    if (!el) return;

    const bottom = isAtBottom();
    setAtBottom(bottom);
    if (bottom) setHasUnseen(false);
    // Scrolling away from the bottom cancels the pin. Without this the list
    // re-pins itself on the next measurement pass and drags the reader back
    // down from wherever they went.
    else stick.current = false;

    // `!anchor.current` holds off the next page until the previous one has been
    // anchored: without it each correction lands inside another request, the
    // pending anchor is overwritten, and the corrections are lost one by one.
    if (el.scrollTop < OLDER_THRESHOLD_PX && hasOlder && !isLoadingOlder && anchor.current === null) {
      anchor.current = el.scrollHeight - el.scrollTop - el.clientHeight;
      anchorArmed.current = false;
      onLoadOlder();
    }
  }, [hasOlder, isAtBottom, isLoadingOlder, onLoadOlder]);

  const totalSize = virtualizer.getTotalSize();

  // Runs on every measurement pass, not only when the rows change: react-virtual
  // re-measures after mount, and each pass moves the anchor row again until the
  // heights settle. It stops re-applying once the offset it wants is the offset
  // the list already has.
  useLayoutEffect(() => {
    const el = scrollRef.current;
    if (!el || rows.length === 0) return;

    const nextFirst = rows[0].key;
    const nextLast = rows[rows.length - 1].key;
    const prepended = firstKey.current !== undefined && firstKey.current !== nextFirst;
    const appended = lastKey.current !== undefined && lastKey.current !== nextLast;
    firstKey.current = nextFirst;
    lastKey.current = nextLast;

    if (!settled.current) {
      settled.current = true;
      stick.current = true;
    }

    if (prepended) anchorArmed.current = true;

    if (anchor.current !== null && anchorArmed.current) {
      const target = Math.max(0, el.scrollHeight - el.clientHeight - anchor.current);
      if (Math.abs(el.scrollTop - target) > 1) {
        el.scrollTop = target;
      } else {
        anchor.current = null;
        anchorArmed.current = false;
      }
      return;
    }

    if (appended && !prepended) {
      if (atBottom) stick.current = true;
      else setHasUnseen(true);
    }

    if (stick.current) {
      if (el.scrollHeight - el.scrollTop - el.clientHeight > 1) el.scrollTop = el.scrollHeight;
      else stick.current = false;
    }
  }, [rows, totalSize, atBottom]);

  useEffect(() => {
    if (!scrollToKey) return;
    const index = rows.findIndex((row) => row.key === scrollToKey);
    if (index === -1) return;
    virtualizer.scrollToIndex(index, { align: 'center' });
    onScrollToKeyHandled?.();
  }, [scrollToKey, rows, virtualizer, onScrollToKeyHandled]);

  const items = virtualizer.getVirtualItems();

  return (
    <div className="flex-1 min-h-0 relative">
      <div
        ref={scrollRef}
        data-qa={qa}
        role="log"
        aria-live="polite"
        aria-label={ariaLabel}
        className="h-full overflow-y-auto px-4 py-4"
        onScroll={handleScroll}
      >
        <div style={{ height: virtualizer.getTotalSize(), position: 'relative', width: '100%' }}>
          {items.map((item) => (
            <div
              key={item.key}
              data-index={item.index}
              ref={virtualizer.measureElement}
              style={{
                position: 'absolute',
                top: 0,
                left: 0,
                width: '100%',
                transform: `translateY(${item.start}px)`,
              }}
            >
              {renderRow(rows[item.index])}
            </div>
          ))}
        </div>
      </div>

      {isLoadingOlder && (
        // An overlay, not a row: a spinner that takes part in layout changes the
        // content height as it appears and disappears, which is exactly the
        // shift the anchor exists to prevent.
        <div className="absolute top-2 left-1/2 -translate-x-1/2 pointer-events-none">
          <div className="w-4 h-4 border-2 border-purple-500/30 border-t-purple-500 rounded-full animate-spin" />
        </div>
      )}

      {hasUnseen && (
        <button
          onClick={() => jumpToBottom('smooth')}
          data-qa="jump-to-latest"
          className="absolute bottom-4 left-1/2 -translate-x-1/2 flex items-center gap-1.5 px-3 py-1.5 rounded-full bg-purple-600 hover:bg-purple-500 text-white text-xs font-medium shadow-lg transition cursor-pointer"
        >
          <ArrowDown className="w-3.5 h-3.5" />
          New messages
        </button>
      )}
    </div>
  );
}
