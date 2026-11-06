import { useCallback, useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { autoUpdate, flip, offset, shift, useFloating, type Side } from '@floating-ui/react-dom';

export const TOOLTIP_ATTRIBUTE = 'data-tooltip';
const SIDE_ATTRIBUTE = 'data-tooltip-side';
const HOVER_CARD_ATTRIBUTE = 'data-hover-card';
const SIDES: readonly Side[] = ['top', 'right', 'bottom', 'left'];

const OPEN_DELAY_MS = 400;
const WARM_FOR_MS = 300;
const MARGIN = 8;
const GAP = 6;
const TOOLTIP_ID = 'app-tooltip';

interface Shown {
  target: HTMLElement;
  text: string;
  side: Side;
  animate: boolean;
}

function tooltipTarget(node: EventTarget | null): HTMLElement | null {
  if (!(node instanceof Element)) return null;
  const target = node.closest<HTMLElement>(`[${TOOLTIP_ATTRIBUTE}]`);
  if (!target?.getAttribute(TOOLTIP_ATTRIBUTE) || target.closest(`[${HOVER_CARD_ATTRIBUTE}]`)) return null;
  return target;
}

export function TooltipLayer() {
  const [shown, setShown] = useState<Shown | null>(null);
  const shownRef = useRef<Shown | null>(null);
  const pending = useRef<{ target: HTMLElement; timer: ReturnType<typeof setTimeout> } | null>(null);
  const hiddenAt = useRef(0);
  const dismissed = useRef<HTMLElement | null>(null);

  const cancel = useCallback(() => {
    if (pending.current) clearTimeout(pending.current.timer);
    pending.current = null;
  }, []);

  const show = useCallback(
    (target: HTMLElement, animate: boolean) => {
      cancel();
      const text = target.getAttribute(TOOLTIP_ATTRIBUTE);
      if (!text || !target.isConnected) return;
      const side = SIDES.find((s) => s === target.getAttribute(SIDE_ATTRIBUTE)) ?? 'top';
      const next = { target, text, side, animate };
      shownRef.current = next;
      setShown(next);
    },
    [cancel],
  );

  const hide = useCallback(() => {
    cancel();
    if (shownRef.current) hiddenAt.current = Date.now();
    shownRef.current = null;
    setShown(null);
  }, [cancel]);

  const { refs, floatingStyles, isPositioned, placement } = useFloating({
    placement: shown?.side ?? 'top',
    strategy: 'fixed',
    middleware: [offset(GAP), flip({ padding: MARGIN }), shift({ padding: MARGIN })],
    whileElementsMounted: (reference, floating, update) =>
      autoUpdate(reference, floating, () => {
        if ((reference as Element).isConnected) update();
        else hide();
      }),
  });

  const isWarm = useCallback(() => !!shownRef.current || Date.now() - hiddenAt.current < WARM_FOR_MS, []);

  useEffect(() => {
    const onPointerOver = (e: PointerEvent) => {
      if (e.pointerType !== 'mouse') return;
      const target = tooltipTarget(e.target);
      if (target !== dismissed.current) dismissed.current = null;
      if (!target || target === dismissed.current) {
        if (shownRef.current || pending.current) hide();
        return;
      }
      if (target === shownRef.current?.target || target === pending.current?.target) return;
      if (isWarm()) {
        show(target, false);
        return;
      }
      if (shownRef.current) hide();
      cancel();
      pending.current = { target, timer: setTimeout(() => show(target, true), OPEN_DELAY_MS) };
    };
    const onPointerOut = (e: PointerEvent) => {
      if (e.pointerType === 'mouse' && !e.relatedTarget) hide();
    };
    const onFocusIn = (e: FocusEvent) => {
      const target = tooltipTarget(e.target);
      if (!target || target !== e.target || target === dismissed.current || !target.matches(':focus-visible'))
        return;
      show(target, !isWarm());
    };
    const onFocusOut = (e: FocusEvent) => {
      if (shownRef.current?.target === e.target) hide();
    };
    const onPointerDown = () => {
      dismissed.current = shownRef.current?.target ?? pending.current?.target ?? null;
      hide();
    };
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Escape' || !shownRef.current) return;
      dismissed.current = shownRef.current.target;
      hide();
    };
    const onScroll = () => {
      if (shownRef.current || pending.current) hide();
    };
    const onBlur = () => hide();

    document.addEventListener('pointerover', onPointerOver);
    document.addEventListener('pointerout', onPointerOut);
    document.addEventListener('focusin', onFocusIn);
    document.addEventListener('focusout', onFocusOut);
    document.addEventListener('pointerdown', onPointerDown, true);
    document.addEventListener('keydown', onKeyDown, true);
    document.addEventListener('scroll', onScroll, true);
    window.addEventListener('blur', onBlur);
    return () => {
      cancel();
      document.removeEventListener('pointerover', onPointerOver);
      document.removeEventListener('pointerout', onPointerOut);
      document.removeEventListener('focusin', onFocusIn);
      document.removeEventListener('focusout', onFocusOut);
      document.removeEventListener('pointerdown', onPointerDown, true);
      document.removeEventListener('keydown', onKeyDown, true);
      document.removeEventListener('scroll', onScroll, true);
      window.removeEventListener('blur', onBlur);
    };
  }, [cancel, hide, isWarm, show]);

  const target = shown?.target ?? null;

  useEffect(() => {
    refs.setReference(target);
    if (!target) return;
    const observer = new MutationObserver(() => {
      const text = target.getAttribute(TOOLTIP_ATTRIBUTE);
      if (!text) {
        hide();
        return;
      }
      setShown((current) => {
        if (current?.target !== target || current.text === text) return current;
        const next = { ...current, text };
        shownRef.current = next;
        return next;
      });
    });
    observer.observe(target, { attributes: true, attributeFilter: [TOOLTIP_ATTRIBUTE] });
    return () => observer.disconnect();
  }, [target, refs, hide]);

  const describes = !!target && target.getAttribute('aria-label') !== shown?.text;

  useEffect(() => {
    if (!target || !describes || target.hasAttribute('aria-describedby')) return;
    target.setAttribute('aria-describedby', TOOLTIP_ID);
    return () => target.removeAttribute('aria-describedby');
  }, [target, describes]);

  if (!shown) return null;

  const side = placement.split('-')[0];

  return createPortal(
    <div
      ref={(node) => refs.setFloating(node)}
      id={TOOLTIP_ID}
      role="tooltip"
      data-qa="tooltip"
      style={{ ...floatingStyles, visibility: isPositioned ? 'visible' : 'hidden' }}
      className="pointer-events-none z-80"
    >
      <div
        data-side={side}
        className={`max-w-64 rounded-md border border-tooltip-line bg-tooltip px-2 py-1 text-xs font-medium leading-snug text-tooltip-fg shadow-lg break-words ${
          shown.animate ? 'tooltip-enter' : ''
        }`}
      >
        {shown.text}
      </div>
    </div>,
    document.body,
  );
}
