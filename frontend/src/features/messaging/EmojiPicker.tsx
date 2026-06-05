import { useEffect, useLayoutEffect, useRef, useState, type RefObject } from 'react';
import { useEscapeToClose } from '@/shared/hooks/useEscapeToClose';

interface Props {
  anchorRef: RefObject<HTMLElement | null>;
  onSelect: (emoji: string) => void;
  onClose: () => void;
}

type PickerCtor = new (props: Record<string, unknown>) => HTMLElement;

export default function EmojiPicker({ anchorRef, onSelect, onClose }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ top: number; left: number } | null>(null);
  const onSelectRef = useRef(onSelect);
  const onCloseRef = useRef(onClose);
  onSelectRef.current = onSelect;
  onCloseRef.current = onClose;
  useEscapeToClose(onClose);

  useEffect(() => {
    let cancelled = false;
    const host = ref.current;
    (async () => {
      const mart = await import('emoji-mart');
      const data = (await import('@emoji-mart/data')).default;
      if (cancelled || !host) return;
      const Picker = mart.Picker as unknown as PickerCtor;
      const el = new Picker({
        data,
        theme: 'dark',
        previewPosition: 'none',
        skinTonePosition: 'search',
        autoFocus: true,
        onEmojiSelect: (e: { native: string }) => {
          onSelectRef.current(e.native);
          onCloseRef.current();
        },
      });
      host.appendChild(el);
    })();
    return () => {
      cancelled = true;
      if (host) host.innerHTML = '';
    };
  }, []);

  useLayoutEffect(() => {
    const anchor = anchorRef.current;
    const el = ref.current;
    if (!anchor || !el) return undefined;
    const compute = () => {
      const a = anchor.getBoundingClientRect();
      const w = el.offsetWidth;
      const h = el.offsetHeight;
      if (!w || !h) return;
      const margin = 8;
      let top = a.top - h - 6;
      if (top < margin) top = a.bottom + 6;
      top = Math.max(margin, Math.min(top, window.innerHeight - h - margin));
      let left = a.right - w;
      left = Math.max(margin, Math.min(left, window.innerWidth - w - margin));
      setPos({ top, left });
    };
    compute();
    const ro = new ResizeObserver(compute);
    ro.observe(el);
    window.addEventListener('resize', compute);
    window.addEventListener('scroll', compute, true);
    return () => {
      ro.disconnect();
      window.removeEventListener('resize', compute);
      window.removeEventListener('scroll', compute, true);
    };
  }, [anchorRef]);

  useEffect(() => {
    const onDown = (e: MouseEvent) => {
      const t = e.target as Node;
      if (ref.current?.contains(t) || anchorRef.current?.contains(t)) return;
      onClose();
    };
    document.addEventListener('mousedown', onDown);
    return () => document.removeEventListener('mousedown', onDown);
  }, [anchorRef, onClose]);

  return (
    <div
      ref={ref}
      style={{
        position: 'fixed',
        top: pos?.top ?? -9999,
        left: pos?.left ?? -9999,
        visibility: pos ? 'visible' : 'hidden',
      }}
      className="z-60"
    />
  );
}
