const GAP = 4;
const MARGIN = 8;

export function positionSuggestion(el: HTMLElement, rect: DOMRect | null | undefined, roomBelow: number) {
  if (!rect) return;
  if (window.innerHeight - rect.bottom < roomBelow) {
    el.style.top = '';
    el.style.bottom = `${window.innerHeight - rect.top + GAP}px`;
  } else {
    el.style.bottom = '';
    el.style.top = `${rect.bottom + GAP}px`;
  }
  const rightmost = window.innerWidth - el.offsetWidth - MARGIN;
  el.style.left = `${Math.max(MARGIN, Math.min(rect.left, rightmost))}px`;
}

export function placeSuggestion(el: HTMLElement, rect: DOMRect | null | undefined, roomBelow: number) {
  positionSuggestion(el, rect, roomBelow);
  requestAnimationFrame(() => positionSuggestion(el, rect, roomBelow));
}
