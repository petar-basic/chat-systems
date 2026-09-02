import { useEffect, useRef, type RefObject } from 'react';
import { createPortal } from 'react-dom';
import { autoUpdate, flip, offset, shift, useFloating } from '@floating-ui/react-dom';
import { useEscapeToClose } from '@/shared/hooks/useEscapeToClose';
import { useThemeStore } from '@/stores/theme';

interface Props {
  anchorRef: RefObject<HTMLElement | null>;
  onSelect: (emoji: string) => void;
  onClose: () => void;
}

type PickerCtor = new (props: Record<string, unknown>) => HTMLElement;

export default function EmojiPicker({ anchorRef, onSelect, onClose }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const theme = useThemeStore((s) => s.resolved);
  const onSelectRef = useRef(onSelect);
  const onCloseRef = useRef(onClose);
  useEffect(() => {
    onSelectRef.current = onSelect;
    onCloseRef.current = onClose;
  });
  useEscapeToClose(onClose);

  const { refs, floatingStyles, isPositioned } = useFloating({
    placement: 'top-end',
    strategy: 'fixed',
    middleware: [offset(6), flip({ padding: 8 }), shift({ padding: 8 })],
    whileElementsMounted: autoUpdate,
  });

  useEffect(() => {
    refs.setReference(anchorRef.current);
  }, [anchorRef, refs]);

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
        theme,
        previewPosition: 'bottom',
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
  }, [theme]);

  useEffect(() => {
    const onDown = (e: MouseEvent) => {
      const t = e.target as Node;
      if (ref.current?.contains(t) || anchorRef.current?.contains(t)) return;
      onClose();
    };
    document.addEventListener('mousedown', onDown);
    return () => document.removeEventListener('mousedown', onDown);
  }, [anchorRef, onClose]);

  return createPortal(
    <div
      ref={(node) => {
        ref.current = node;
        refs.setFloating(node);
      }}
      style={{ ...floatingStyles, visibility: isPositioned ? 'visible' : 'hidden' }}
      className="z-60"
    />,
    document.body,
  );
}
